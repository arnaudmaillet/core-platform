# `traffic-redis` — Redis-Lease Distributed Backend for `traffic`

## 🎯 Overview & Service Role

Implements [`traffic::QuotaBackend`](../traffic) so a `mode = "distributed"` rate-limit
profile enforces a **fleet-global** budget — not just per-replica — **without a Redis
round-trip per request**. Each replica *leases* a chunk of the global per-window budget and
serves it locally, only crossing to Redis when its chunk runs out (or to learn the window is
spent). Backend I/O is therefore amortized over `burst` requests per key per replica.

`transport` depends only on the `QuotaBackend` trait (in pure `traffic`); the serving binary
injects this implementation, so the transport layer links no Redis.

## 📐 How the lease works

```
QuotaBackend (trait, in `traffic`)
 └─ RedisLeaseBackend
      ├─ LeaseBook    — per-key local lease cache + windowed-budget algorithm (pure)
      └─ ClaimSource  — atomic "lease N tokens" seam
           └─ RedisClaimSource — one atomic Lua script (single key → cluster-slot-safe)
```

- **Window budget** = `ceil(rps × lease_ms / 1000)` global tokens per `lease_ms` window.
- **Lease chunk** = `burst` (capped at the budget): larger = fewer Redis claims, coarser
  cross-replica fairness.
- **Exhausted-window short-circuit**: once a window's global budget is spent it stays spent,
  so an over-budget flood is served locally — it does **not** hammer Redis.
- **Failure policy**: a claim failure surfaces as `QuotaError`; the transport layer maps it to
  the profile's `on_backend_error` (`fail_open` → degrade to the local governor;
  `fail_closed` → reject). Requests served from an existing lease never touch Redis, so a
  Redis blip only affects refills.

The pure `LeaseBook` algorithm is unit-tested against an in-memory `ClaimSource`; the live
Lua path is integration-tested behind a feature gate.

## 🔌 Key API

```rust
let backend = Arc::new(RedisLeaseBackend::new(redis_client));
// hand to the gRPC server:
builder.with_traffic_backend(Arc::clone(&backend) as Arc<dyn traffic::QuotaBackend>);
// periodic memory bound for per_caller keyspaces:
backend.prune(lease_ms);
```

See the [binary rollout checklist](../infra-config/README.md#-binary-bootstrap--rollout-checklist).

## 🛠️ Local Development

```bash
cargo test -p traffic-redis                                   # hermetic: lease algorithm vs a fake claim source
cargo test -p traffic-redis --features integration-traffic-redis   # live Redis (needs Docker)
cargo clippy -p traffic-redis -- -D warnings
```
