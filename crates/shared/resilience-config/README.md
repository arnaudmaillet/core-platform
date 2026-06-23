# `resilience-config` — Externalized Configuration & Hot-Reload for `resilience`

## 🎯 Overview & Service Role

`resilience-config` is the **policy / IO layer** that bridges the pure [`resilience`](../resilience) middleware crate to an externalized configuration paradigm. `resilience` owns the *mechanism* (Tower layers + serde-able wire types); this crate owns everything the mechanism must **not** depend on:

- **File IO + parsing** — reads `infrastructure.toml` into typed config.
- **Validation** — rejects structurally-valid-but-semantically-broken configs *before* they reach the data path (fail-closed).
- **Fleet bindings** — resolves a downstream dependency name (`"post-command"`) to a named class-of-service profile (`"critical"`).
- **Hot-reload** — a `notify`-based watcher that re-applies the file on change via lock-free `ArcSwap` swaps, with no restart and no torn in-flight futures.

Keeping this separate is deliberate: `resilience` links no `notify`, no `toml`, no filesystem. Services depend on **both** — `resilience` for the layers, `resilience-config` for where the numbers come from.

> **Status:** complete and tested (parse/resolve/bindings, fail-closed validation, and a real-filesystem end-to-end hot-reload test).

---

## 📐 Architecture & Concepts

```text
infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ ResilienceRegistry
                                     ▲                                │
                                notify event                  profile_for("post-command")
                              spawn_watcher ──validate──▶ apply()     ▼
                              (single writer, fail-closed)     ResilienceProfile ──▶ Tower layers
```

### Two representations (from `resilience`)

| | Type | Role |
|---|---|---|
| **Wire** | `ResilienceProfileSpec` | Flat, serde-friendly. Pure data parsed from TOML. |
| **Runtime** | `ResilienceProfile` | Holds shared `Arc<ArcSwap<_>>` handles the Tower layers read each `call()`. |

### Topology vs. contents

- **Topology** — which profiles exist and which dependency binds to which — is **fixed at boot**. Tower layers capture a profile's handles when built, so re-binding requires rebuilding those layers.
- **Contents** — the timeout + circuit-breaker thresholds — **hot-reload** via `apply()`. This is the incident-critical path (tighten a deadline, lower a trip threshold fleet-wide in seconds).

Changing the profile set or bindings requires a restart; changing a bound profile's values does not.

### Hot-reload safety

- **Single writer.** All swaps happen in one spawned task — reloads never race.
- **Fail-closed.** Parse or validation errors are logged and the previous config is kept. A bad push cannot take the fleet down.
- **K8s-aware.** ConfigMaps are mounted via an atomically-swapped `..data` symlink that replaces the file's inode. The watcher therefore watches the **parent directory**, not the file path, or it would go deaf after the first swap.
- **Coalesced.** Editors and atomic swaps emit event bursts; the watcher drains them and reloads once.

---

## 🔌 Public Interfaces & API Contract

```rust
// schema.rs — wire types
pub struct InfrastructureConfig { pub resilience: ResilienceSection }
pub struct ResilienceSection {
    pub profiles: HashMap<String, ResilienceProfileSpec>, // catalog: name -> spec
    pub bindings: HashMap<String, String>,                // dependency -> profile name
    pub default_profile: String,                          // fallback (default: "standard")
}
impl InfrastructureConfig { pub fn from_toml(raw: &str) -> Result<Self, ConfigError>; }
impl ResilienceSection   { pub fn validate(&self) -> Result<(), ConfigError>; }

// registry.rs — boot-time resolution + hot-apply
impl ResilienceRegistry {
    pub fn from_config(InfrastructureConfig) -> Result<Self, ConfigError>;
    pub fn profile_for(&self, dependency: &str) -> ResilienceProfile;  // binding, else default
    pub fn profile(&self, name: &str) -> Option<ResilienceProfile>;
    pub fn apply(&self, InfrastructureConfig) -> Result<(), ConfigError>; // validates, then swaps
}

// watcher.rs — hot-reload
pub fn load_from_path(path: &Path) -> Result<InfrastructureConfig, ConfigError>;
pub fn spawn_watcher(path: PathBuf, registry: Arc<ResilienceRegistry>)
    -> Result<notify::RecommendedWatcher, ConfigError>;   // KEEP the guard alive
```

**Validation invariants** (enforced before resolve *and* every hot-swap):
- `default_profile` and every binding target must reference a defined profile.
- `failure_threshold`, `success_threshold`, `half_open_max_calls` > 0; `timeout` > 0.
- backoff `max_ms >= base_ms`.

`ConfigError`: `Io` · `Toml` · `Watch` · `Validation(String)`.

---

## 📦 Integration & Usage

```toml
[dependencies]
resilience-config = { workspace = true }
```

```rust
use std::sync::Arc;
use resilience_config::{InfrastructureConfig, ResilienceRegistry, spawn_watcher};

// Boot: load, resolve, start watching.
let registry = Arc::new(ResilienceRegistry::from_config(
    InfrastructureConfig::from_toml(&std::fs::read_to_string("infrastructure.toml")?)?,
)?);
let _watcher = spawn_watcher("infrastructure.toml".into(), Arc::clone(&registry))?; // keep alive

// Resolve a dependency's profile and build a hot-reloadable client stack.
let channel = GrpcClientBuilder::new(
    GrpcClientConfig::new("https://post:50051").with_dependency("post-command")
)
.build_from_registry(&registry)
.await?;
```

See [`examples/infrastructure.toml`](examples/infrastructure.toml) for the full `standard` / `critical` / `aggressive` catalog and bindings.

---

## 🛠️ Local Development

```bash
cargo test -p resilience-config            # unit + integration (incl. real-fs hot-reload)
cargo clippy -p resilience-config -- -D warnings
```

No external services required — the hot-reload test writes to a temp directory and watches it.

---

## 🚨 Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Config never reloads | The `RecommendedWatcher` guard was dropped | Bind it to a long-lived variable (`let _watcher = …`); dropping it stops the watch. |
| Reload ignored after first change (in K8s) | Watching the file path, not the dir | This crate watches the parent dir for exactly this reason; ensure the mount path's parent is accessible. |
| `Validation` error on a known-good file | An invariant violated (zero threshold, `max_ms < base_ms`, dangling binding) | Read the error message — it names the offending profile/field. Previous config stays live. |
| Profile change not picked up | It's a *topology* change (added/removed profile or re-binding) | Restart — only profile *contents* hot-reload. |
