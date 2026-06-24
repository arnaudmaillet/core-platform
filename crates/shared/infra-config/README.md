# `infra-config` — Externalized Configuration & Hot-Reload for Fleet Infrastructure

## 🎯 Overview & Service Role

`infra-config` is the **policy / IO layer** that bridges the pure middleware crates (starting with [`resilience`](../resilience)) to an externalized configuration paradigm. The middleware crates own the *mechanism* (Tower layers, cache adapters, serde-able wire types); this crate owns everything the mechanism must **not** depend on:

- **File IO + parsing** — reads `infrastructure.toml` into typed, per-section config.
- **Validation** — rejects structurally-valid-but-semantically-broken configs *before* they reach the data path (fail-closed).
- **Fleet bindings** — resolves a dependency/namespace name (`"post-command"`, `"profile-view"`) to a named class-of-service profile (`"critical"`, `"hot"`).
- **Hot-reload** — a `notify`-based watcher that re-applies the file on change via lock-free `ArcSwap` swaps, with no restart and no torn in-flight futures.

It is **multi-tenant**: each infrastructure category is a `[section]` sharing one catalog shape, one watcher, and one fail-closed reload path. Today it ships `[resilience]` (timeouts, circuit breakers, retries) and `[cache]` (TTL profiles); `[traffic]` (rate limiting) and `[telemetry]` (log level, sampling) are planned tenants.

Keeping this separate is deliberate: the middleware crates link no `notify`, no `toml`, no filesystem. Services depend on **both** — the middleware for the layers/adapters, `infra-config` for where the numbers come from.

> **Status:** complete and tested — `[resilience]` + `[cache]` sections, the generic catalog, the aggregate `InfraRegistry`, fail-closed cross-section validation, and a real-filesystem end-to-end hot-reload test.

---

## 📐 Architecture & Concepts

```text
infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ InfraRegistry
                                     ▲                          ├─ ResilienceRegistry ─▶ Tower layers
                                notify event                    └─ CacheRegistry ──────▶ cache adapters
                              spawn_watcher ──reload──▶ InfraRegistry::apply()
                              (single writer, fail-closed, all-sections-or-nothing)
```

### The catalog shape (every section shares it)

A *section* is a catalog of named class-of-service profiles plus a binding table mapping each dependency/namespace to a profile, with a default for the unbound. The reference-integrity check (`catalog::validate_bindings`) and the resolved lookup (`catalog::Catalog<L>`) are written **once** and reused by every section — adding a tenant means adding a spec + live type, not re-implementing resolution.

### Two representations (per section)

| | Type (resilience / cache) | Role |
|---|---|---|
| **Wire** | `ResilienceProfileSpec` / `CacheProfileSpec` | Flat, serde-friendly. Pure data parsed from TOML. |
| **Runtime** | `ResilienceProfile` / `CacheProfile` | Holds shared `Arc<ArcSwap<_>>` handles the data path reads each call/write. |

### Topology vs. contents

- **Topology** — which sections/profiles exist and which dependency binds to which — is **fixed at boot**. Callers capture a profile's handle when wired, so re-binding (or adding a section) requires a restart.
- **Contents** — a profile's values (timeout, trip threshold, TTL) — **hot-reload**. This is the incident-critical path (tighten a deadline, widen a TTL to ride out a stampede, fleet-wide in seconds).

### Hot-reload safety

- **Single writer.** All swaps happen in one spawned task — reloads never race.
- **Fail-closed, all-or-nothing.** `InfraRegistry::apply` validates *every* present section before swapping *any*; a bad push to one section leaves all sections on their previous values.
- **K8s-aware.** ConfigMaps are mounted via an atomically-swapped `..data` symlink that replaces the file's inode. The watcher therefore watches the **parent directory**, not the file path, or it would go deaf after the first swap.
- **Coalesced.** Editors and atomic swaps emit event bursts; the watcher drains them and reloads once.

---

## 🔌 Public Interfaces & API Contract

```rust
// schema.rs — top-level document; new sections are Option<…> for backward compatibility.
pub struct InfrastructureConfig {
    pub resilience: ResilienceSection,
    pub cache: Option<CacheSection>,
}
impl InfrastructureConfig {
    pub fn from_toml(raw: &str) -> Result<Self, ConfigError>;
    pub fn validate(&self) -> Result<(), ConfigError>;   // every present section
}

// reload.rs — the watcher's target, decoupled from any section's shape.
pub trait Reloadable: Send + Sync + 'static {
    fn reload(&self, raw: &str) -> Result<(), ConfigError>;   // parse + validate + swap, fail-closed
}

// infra.rs — the aggregate registry (drive this in production).
impl InfraRegistry {
    pub fn from_config(InfrastructureConfig) -> Result<Self, ConfigError>;
    pub fn resilience(&self) -> Arc<ResilienceRegistry>;
    pub fn cache(&self) -> Option<Arc<CacheRegistry>>;
    pub fn apply(&self, InfrastructureConfig) -> Result<(), ConfigError>;
}
// impl Reloadable for InfraRegistry      — all sections
// impl Reloadable for ResilienceRegistry — standalone, resilience-only deployments

// registry.rs / cache.rs — per-section resolution + hot-apply.
impl ResilienceRegistry { pub fn profile_for(&self, dependency: &str) -> ResilienceProfile; /* … */ }
impl CacheRegistry      { pub fn profile_for(&self, namespace: &str) -> CacheProfile;        /* … */ }

// watcher.rs — generic over the target.
pub fn load_from_path(path: &Path) -> Result<InfrastructureConfig, ConfigError>;
pub fn spawn_watcher<R: Reloadable>(path: PathBuf, target: Arc<R>)
    -> Result<notify::RecommendedWatcher, ConfigError>;   // KEEP the guard alive
```

**Validation invariants** (enforced before resolve *and* every hot-swap):
- Every section: `default_profile` and every binding target must reference a defined profile.
- `[resilience]`: thresholds / `half_open_max_calls` / `timeout` > 0; backoff `max_ms >= base_ms`.
- `[cache]`: `ttl_secs` > 0.

`ConfigError`: `Io` · `Toml` · `Watch` · `Validation(String)`.

---

## 📦 Integration & Usage

```toml
[dependencies]
infra-config = { workspace = true }
```

```rust
use std::sync::Arc;
use infra_config::{InfraRegistry, InfrastructureConfig, spawn_watcher};

// Boot: load, resolve every section, start watching.
let registry = Arc::new(InfraRegistry::from_config(
    InfrastructureConfig::from_toml(&std::fs::read_to_string("infrastructure.toml")?)?,
)?);
let _watcher = spawn_watcher("infrastructure.toml".into(), Arc::clone(&registry))?; // keep alive

// Resilience: build a hot-reloadable gRPC client stack from a binding.
let channel = GrpcClientBuilder::new(
    GrpcClientConfig::new("https://post:50051").with_dependency("post-command")
)
.build_from_registry(&registry.resilience())
.await?;

// Cache: hand a service its resolved TTL profiles (the service resolves its own namespaces).
let app = profile::app::App::build(backends, registry.cache().expect("[cache] configured")).await?;
```

A resilience-only deployment can drive the same watcher with `Arc<ResilienceRegistry>` directly — both implement `Reloadable`. See [`examples/infrastructure.toml`](examples/infrastructure.toml) for the full `[resilience]` + `[cache]` catalogs and bindings.

---

## 🛠️ Local Development

```bash
cargo test -p infra-config            # unit + integration (incl. real-fs hot-reload)
cargo clippy -p infra-config -- -D warnings
```

No external services required — the hot-reload test writes to a temp directory and watches it.

---

## 🚨 Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Config never reloads | The `RecommendedWatcher` guard was dropped | Bind it to a long-lived variable (`let _watcher = …`); dropping it stops the watch. |
| Reload ignored after first change (in K8s) | Watching the file path, not the dir | This crate watches the parent dir for exactly this reason; ensure the mount path's parent is accessible. |
| `Validation` error on a known-good file | An invariant violated (zero threshold/TTL, `max_ms < base_ms`, dangling binding) | Read the error message — it names the section and offending profile/field. Previous config stays live. |
| One section's bad value blocks a different section's change | Reload is all-or-nothing across sections (fail-closed) | Fix the rejected section; the whole document re-applies together. |
| Profile change not picked up | It's a *topology* change (added/removed profile or section, or a re-binding) | Restart — only profile *contents* hot-reload. |
