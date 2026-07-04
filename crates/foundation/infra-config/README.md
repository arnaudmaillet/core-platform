# `infra-config` — Externalized configuration & hot-reload for the fleet's infrastructure layers

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — the policy / IO layer feeding the pure middleware crates |
> | **Package** | `infra-config` (dir: `crates/foundation/infra-config`) |
> | **Consumed by** | `service-runtime` (loads + watches); services read resolved `[cache]` profiles |
> | **Depends on** | `notify`, `toml`, `serde`, `arc-swap` |
> | **Stability** | evolving (new `[section]`s are added over time) |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`infra-config` is the **policy / IO layer** that bridges the pure middleware crates (e.g.
[`resilience`](../resilience)) to an externalized configuration paradigm. The middleware crates own
the *mechanism* (Tower layers, cache adapters, serde wire types); this crate owns everything the
mechanism must **not** depend on: file IO + parsing (`infrastructure.toml` → typed config),
fail-closed validation, fleet bindings (resolve a dependency/namespace name to a class-of-service
profile), and `notify`-based hot-reload via lock-free `ArcSwap` swaps.

**Architectural boundary** — the middleware crates link **no** `notify`, `toml`, or filesystem.
Services depend on **both**: the middleware for the layers/adapters, `infra-config` for where the
numbers come from. It ships four sections: `[resilience]`, `[cache]`, `[traffic]`, `[telemetry]`.

---

## 📐 Architecture & key decisions

```
infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ InfraRegistry
                                     ▲                          ├─ ResilienceRegistry ─▶ Tower layers
                                notify event                    ├─ CacheRegistry ──────▶ cache adapters
                              spawn_watcher                     ├─ TrafficRegistry ────▶ ingress limiter
                              ──reload──▶ InfraRegistry::apply() └─ TelemetryRegistry ──▶ TelemetrySink (live)
                              (single writer, fail-closed, all-sections-or-nothing)
```

- **One catalog shape, written once** — every section is a catalog of named profiles + a binding
  table (dependency → profile, with a default). `catalog::validate_bindings` and `Catalog<L>` are
  shared by all sections, so adding a tenant means adding a spec + live type, not re-implementing
  resolution.
- **Two representations per section** — a flat serde **Wire** type (`…ProfileSpec`, parsed from TOML)
  and a **Runtime** type (`…Profile`) holding `Arc<ArcSwap<_>>` handles the data path reads each call.
- **Topology fixed at boot, contents hot-reload** — *which* sections/profiles exist and *which*
  dependency binds where is captured when wired (re-binding needs a restart). A profile's *values*
  (timeout, TTL) hot-reload — that's the incident-critical path.
- **Hot-reload safety** — all swaps happen in **one** spawned task (no races); `apply` validates
  *every* present section before swapping *any* (**fail-closed, all-or-nothing**); the watcher watches
  the **parent directory** not the file (K8s ConfigMaps swap the `..data` symlink inode, so a
  file-path watch goes deaf after the first change); event bursts are coalesced into one reload.

---

## 🔌 Public API & contract

```rust
// schema.rs — top-level document; new sections are Option<…> for backward compatibility.
impl InfrastructureConfig {
    pub fn from_toml(raw: &str) -> Result<Self, ConfigError>;
    pub fn validate(&self) -> Result<(), ConfigError>;        // every present section
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
// impl Reloadable for InfraRegistry (all sections) and for ResilienceRegistry (resilience-only)

// watcher.rs — generic over the target.
pub fn load_from_path(path: &Path) -> Result<InfrastructureConfig, ConfigError>;
pub fn spawn_watcher<R: Reloadable>(path: PathBuf, target: Arc<R>) -> Result<notify::RecommendedWatcher, ConfigError>; // KEEP the guard alive
```

> **Validation invariants** (before resolve *and* every hot-swap): every section's `default_profile`
> and binding targets must reference a defined profile; `[resilience]` thresholds / `half_open_max_calls`
> / `timeout` > 0 and backoff `max_ms >= base_ms`; `[cache]` `ttl_secs` > 0. `ConfigError`: `Io` ·
> `Toml` · `Watch` · `Validation(String)`.

---

## 📦 Integration

```toml
[dependencies]
infra-config = { workspace = true }
```

```rust
use std::sync::Arc;
use infra_config::{InfraRegistry, InfrastructureConfig, spawn_watcher};

let registry = Arc::new(InfraRegistry::from_config(
    InfrastructureConfig::from_toml(&std::fs::read_to_string("infrastructure.toml")?)?,
)?);
let _watcher = spawn_watcher("infrastructure.toml".into(), Arc::clone(&registry))?; // keep alive!

// resilience: hot-reloadable client stack from a binding; cache: hand a service its resolved TTLs
let app = profile::app::App::build(backends, registry.cache().expect("[cache] configured")).await?;
```

See [`examples/infrastructure.toml`](examples/infrastructure.toml) for the full catalogs + bindings.

---

## ⚙️ Configuration & feature flags

No environment variables and no cargo features — configuration *is* the file
(`infrastructure.toml`, path supplied by the caller). The serving binary (`service-runtime`) loads the
document, spawns the watcher, and registers the traffic layer + telemetry sink.

---

## 🧪 Testing

```bash
cargo test   -p infra-config           # unit + a real-filesystem hot-reload integration test
cargo clippy -p infra-config --all-targets
```

No external services — the hot-reload test writes to a temp dir and watches it.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. Config never reloads.**
The `RecommendedWatcher` guard was dropped. Bind it to a long-lived variable (`let _watcher = …`);
dropping it stops the watch.

**2. Reload ignored after the first change (in K8s).**
Something watched the file *path* instead of the parent dir. This crate watches the parent dir for
exactly this reason (ConfigMaps swap the `..data` symlink inode) — ensure the mount's parent is
accessible.

**3. `Validation` error on a known-good file.**
An invariant was violated (zero threshold/TTL, `max_ms < base_ms`, dangling binding). Read the message
— it names the section and field. The previous config stays live (fail-closed).

**4. A profile change wasn't picked up despite a reload.**
It's a *topology* change (added/removed profile or section, or a re-binding) — only profile *contents*
hot-reload. Restart to apply topology changes.
