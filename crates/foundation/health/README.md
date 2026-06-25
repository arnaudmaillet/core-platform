# `health` — Liveness/readiness probe abstraction, a graph-leaf so storage exposes probes without the runtime

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — graph-leaf probe abstraction (decouples storage from the runtime) |
> | **Package** | `health` (dir: `crates/foundation/health`) |
> | **Consumed by** | `service-runtime` (polls probes → gRPC health), storage crates (`scylla`/`redis`/`postgres` expose probes) |
> | **Depends on** | `async-trait`, `anyhow` |
> | **Stability** | stable contract |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`health` defines the liveness/readiness probe abstraction the fleet uses to drive a service's gRPC
health status. `HealthProbe` is a **graph-leaf**: storage crates expose ready-made probes over their
live clients (depending only on this foundation crate, never on the runtime), and the runtime polls
those probes to drive health — **without either side depending on the other**.

**Architectural boundary** — it owns only the probe *contract*. It does not open connections, does not
know about gRPC, and does not schedule polling; the runtime does the polling, the storage crates do
the checking.

---

## 📐 Architecture & key decisions

```
storage crate (scylla/redis/postgres)         service-runtime
   exposes  impl HealthProbe  ───────────────►  polls .check() each readiness tick
                     ▲                                   │ any Err
              both depend only on `health`               ▼
                                              demote service to NOT_SERVING until a tick clears it
```

- **Graph-leaf by design** — placing the trait in a tiny foundation crate is what lets a storage crate
  publish a probe and the runtime consume it with no edge between storage and runtime. Moving the trait
  into either side would reintroduce that coupling.
- **Probes must be cheap** — `check()` runs on **every** readiness tick, so a probe is a light
  reachability ping (e.g. `system.local`, `PING`, `SELECT 1`), never a heavy query.
- **Fail-to-`NOT_SERVING`, self-clearing** — any `Err` from any probe demotes the whole service until a
  subsequent tick succeeds; there is no sticky failure state to reset.

---

## 🔌 Public API & contract

```rust
#[async_trait]
pub trait HealthProbe: Send + Sync + 'static {
    fn name(&self) -> &str;                       // short id for logs, e.g. "scylla" / "redis"
    async fn check(&self) -> anyhow::Result<()>;  // Ok = reachable; any Err demotes to NOT_SERVING
}

/// A HealthProbe backed by an async closure, for a bespoke check not provided by a storage crate.
/// The closure is `Fn` (re-run every tick), typically capturing a cloned client handle.
pub struct FnProbe<F>;
impl<F> FnProbe<F> { pub fn new(name: &'static str, check: F) -> Self; }
```

> **Contract notes:** `check()` is polled every tick — keep it cheap and idempotent. `name()` is for
> logs only. Storage crates ship their own `HealthProbe` impls (see each crate's `health::probe`);
> `FnProbe` is the escape hatch for everything else.

---

## 📦 Integration

```toml
[dependencies]
health = { workspace = true }
```

```rust
use health::{HealthProbe, FnProbe};

// A bespoke probe over any client handle:
let probe = FnProbe::new("elasticsearch", move || {
    let client = client.clone();
    async move { client.ping().await.map_err(Into::into) }
});

// The runtime collects Vec<Box<dyn HealthProbe>> from the service and polls them each tick.
```

---

## ⚙️ Configuration & feature flags

None — no environment variables, no cargo features. Polling cadence and gRPC wiring belong to
`service-runtime`.

---

## 🧪 Testing

```bash
cargo test   -p health
cargo clippy -p health --all-targets
```

Pure library — no external services required.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. A service flaps to `NOT_SERVING` under load.**
A probe's `check()` is too heavy and times out on a busy tick. Probes must be light reachability pings,
not real queries — any `Err` (including a timeout) demotes the whole service until the next tick.

**2. My `FnProbe` closure won't compile / captures a moved value.**
The closure is `Fn` (re-invoked every tick), so it must be re-callable — capture a **cloned** client
handle and `clone()` it again inside the `async move` block, as in the example above.

**3. Where do `scylla`/`redis`/`postgres` probes come from?**
Each storage crate exposes its own `HealthProbe` over its client (its `health::probe`). Use those
directly; reach for `FnProbe` only for dependencies without a ready-made probe.
