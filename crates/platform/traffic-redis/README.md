# `traffic-redis` — Redis-lease distributed backend for the `traffic` rate limiter

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `platform` — the `QuotaBackend` that makes `traffic` profiles fleet-global (Step 2) |
> | **Package** | `traffic-redis` (dir: `crates/platform/traffic-redis`) |
> | **Consumed by** | `transport` (wired as the `QuotaBackend` for `distributed` traffic profiles) |
> | **Depends on** | `traffic`, `redis-storage`, `fred` (`i-scripts`), `async-trait`, `dashmap` |
> | **Stability** | evolving |
> | **Feature flags** | `integration-traffic-redis` (live-Redis test; off by default) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`traffic-redis` implements [`traffic::QuotaBackend`](../../foundation/traffic) so `distributed`
profiles enforce a **fleet-global** budget **without a Redis round-trip per request**: each replica
leases a chunk of the global per-window budget and serves it locally, only crossing to Redis when its
chunk is exhausted (or to discover the window is fully spent).

**Architectural boundary** — it provides only the distributed *quota backend*. The limiter mechanism,
config types, and `check()` decision live in [`traffic`](../../foundation/traffic); the gRPC glue that
wires this backend and maps decisions lives in [`transport`](../transport). Backend I/O is amortized
over `burst` requests per key per replica; a fully-spent window is cached locally so an over-budget
flood does not hammer Redis.

---

## 📐 Architecture & key decisions

```
traffic::QuotaBackend (trait, in `traffic`)
  └─ RedisLeaseBackend
       ├─ LeaseBook       — local per-key lease cache + windowed-budget algorithm (PURE)
       └─ ClaimSource     — atomic "lease N tokens" seam
            └─ RedisClaimSource — one Lua script (single key → cluster-slot-safe)
```

- **Lease-a-chunk, not check-per-request** — the whole point: a replica claims `burst` tokens at once
  and serves them locally, so the hot path rarely touches the network. Backend I/O scales with refills,
  not requests.
- **Pure algorithm behind a `ClaimSource` seam** — `LeaseBook` (the windowed-budget logic) is
  transport- and Redis-agnostic and unit-tested against an **in-memory** `ClaimSource`;
  `RedisClaimSource` is the thin live implementation. This is what keeps the unit suite hermetic.
- **One Lua script, single key** — the atomic claim is a single-key Lua script, so it is
  **cluster-slot-safe** (no `CROSSSLOT`).
- **Fail-soft via policy** — a claim failure surfaces as `traffic::QuotaError`; `transport` maps it to
  the profile's `on_backend_error` policy (degrade to the local limiter, or reject). Requests served
  from an existing local lease never touch Redis, so a Redis blip only affects refills.

---

## 🔌 Public API & contract

```rust
pub use claim::ClaimSource;
pub use lease::{window_budget, LeaseBook};
pub use redis::{RedisClaimSource, RedisLeaseBackend};

pub trait ClaimSource: Send + Sync { /* atomic "lease N tokens for key in window" */ }

pub struct LeaseBook;                                  // pure local lease cache + windowed-budget algorithm
impl LeaseBook {
    pub fn new() -> Self;
    pub async fn check<C: ClaimSource>(&self, /* key, quota, now, source */) -> /* decision */;
    pub fn prune(&self, now_ms: u64, lease_ms: u64);   // evict idle per-key leases
    pub fn tracked_keys(&self) -> usize;
}
pub fn window_budget(rps: u32, lease_ms: u64) -> u64;  // tokens available in one lease window

pub struct RedisClaimSource;  impl { pub fn new(client: RedisClient) -> Self; }            // the one Lua script
pub struct RedisLeaseBackend; impl traffic::QuotaBackend for RedisLeaseBackend { /* … */ }
impl RedisLeaseBackend { pub fn new(client: RedisClient) -> Self; pub fn prune(&self, lease_ms: u64); pub fn tracked_keys(&self) -> usize; }
```

> **Contract notes:** `RedisLeaseBackend` is wired into the gRPC server as
> `Arc<dyn traffic::QuotaBackend>` (see `transport`). Run its `prune(lease_ms)` on a timer (the
> `lease_ms` must match the profiles' lease window) to bound per-key memory. Only `distributed`
> profiles consult the backend; `local` profiles never touch it.

---

## 📦 Integration

```toml
[dependencies]
traffic-redis = { workspace = true }
```

```rust
// transport server wiring (distributed mode):
let backend = Arc::new(traffic_redis::RedisLeaseBackend::new(redis_client));
builder = builder
    .with_traffic(Arc::clone(&traffic))
    .with_traffic_backend(Arc::clone(&backend) as Arc<dyn traffic::QuotaBackend>);

let lease_ms = 1_000; // match the profiles' lease window
tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop { tick.tick().await; backend.prune(lease_ms); }
});
```

---

## ⚙️ Configuration & feature flags

No environment variables of its own — it takes a `RedisClient` (configured via `redis-storage`) and is
driven by the `distributed` `[traffic]` profiles resolved through `infra-config`.

**Feature flags:** `integration-traffic-redis` — gates the live-Redis test (Docker required). Off by
default so the unit suite (which exercises the lease algorithm against an in-memory `ClaimSource`) stays
hermetic. `fred` is built with `i-scripts` for the Lua claim script.

---

## 🧪 Testing

```bash
cargo test   -p traffic-redis                                   # hermetic — LeaseBook vs in-memory ClaimSource
cargo test   -p traffic-redis --features integration-traffic-redis   # live Redis (Docker)
cargo clippy -p traffic-redis --all-targets
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. A `distributed` profile behaves like a per-replica limit.**
No backend is wired — `distributed` profiles **degrade to the local governor** when no `QuotaBackend`
is supplied. Wire `RedisLeaseBackend` via `with_traffic_backend(...)` at boot.

**2. Per-key memory grows over time.**
`LeaseBook` retains one entry per active key. Call `RedisLeaseBackend::prune(lease_ms)` on a timer
(matching the lease window); check `tracked_keys()` to size the cadence.

**3. Effective limit is looser/tighter than the configured rps.**
`window_budget(rps, lease_ms)` sets how many tokens a replica claims per lease — the `lease_ms` passed
to `prune` must match the profile's lease window, or accounting drifts. Keep them equal.

**4. `CROSSSLOT` error on a Redis Cluster.**
Shouldn't happen — the claim is a single-key Lua script (slot-safe) by design. If you see it, a caller
constructed a multi-key operation outside `RedisClaimSource`; keep the claim to the one script.
