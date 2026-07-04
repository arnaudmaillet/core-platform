# `traffic-redis` — Domain & Functional Contract

> The distributed backend for `traffic`: it answers *"how do replicas share one fleet-global rate budget without a Redis round-trip per request?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | A `QuotaBackend` that makes `traffic` `distributed` profiles enforce a fleet-global budget via amortized leasing (Step 2) |
> | **Layer** | `platform` — the IO half of `traffic` (the pure mechanism is the foundation crate) |
> | **Subdomain class** | **Generic** — a leased-budget limiter backend; leverage is the amortization, not the algebra |
> | **Primary abstraction(s)** | `RedisLeaseBackend` + `LeaseBook` + `ClaimSource` (`traffic_redis`) |
> | **Footprint** | IO/stateful — a local lease cache + a single-key Lua claim script against Redis |
> | **Failure posture** | **fail-soft via policy** — a claim failure surfaces as `QuotaError`; `transport` applies `on_backend_error` (degrade or reject) |
> | **Depends on** | `traffic`, `redis-storage`, `fred` (`i-scripts`), `async-trait`, `dashmap` |
> | **Consumed by** | `transport` (wired as the `QuotaBackend` for `distributed` profiles) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `traffic-redis` is the fleet's authority for **distributed rate-limit budget**: it answers
**"what is each replica's slice of the global per-window budget, refilled only when its local lease runs
out?"** — implementing `traffic::QuotaBackend` so `distributed` profiles enforce a fleet-global limit without a
network hop per request.

**The hard problem.** A naïve fleet-global limiter does one Redis round-trip per request — a latency and load
amplifier. `traffic-redis` instead leases a *chunk* (`burst` tokens) per key per replica and serves it locally,
crossing to Redis only on refill or to discover a spent window. The hot path rarely touches the network; a
fully-spent window is cached locally so an over-budget flood does not hammer Redis.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Own the limiter mechanism / config / `check()` decision → those are `traffic` (foundation).
- ❌ Own the gRPC glue (key extraction, decision mapping, `on_backend_error` policy) → that is `transport`.
- ❌ Serve `local` profiles → only `distributed` profiles consult the backend.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Lease | A chunk of the global budget a replica claims and serves locally | `LeaseBook` |
| Claim source | The atomic "lease N tokens for key in window" seam | `ClaimSource`, `RedisClaimSource` |
| Window budget | Tokens available in one lease window for a given rps | `window_budget(rps, lease_ms)` |
| Lease backend | The `QuotaBackend` impl wiring the book to Redis | `RedisLeaseBackend` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `RedisLeaseBackend` | `QuotaBackend` impl | Wired as `Arc<dyn traffic::QuotaBackend>` in `transport`; `prune(lease_ms)` bounds memory |
| `LeaseBook` | pure algorithm | The local lease cache + windowed-budget logic; transport- and Redis-agnostic, unit-tested |
| `ClaimSource` | trait (seam) | The atomic-claim contract; an in-memory impl keeps the unit suite hermetic |
| `RedisClaimSource` | thin adapter | One single-key Lua script → cluster-slot-safe (no `CROSSSLOT`) |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The distributed *quota backend*: the lease algorithm (`LeaseBook`), the claim seam (`ClaimSource`), and the
  Redis adapter (`RedisClaimSource` / `RedisLeaseBackend`).

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| The limiter mechanism + `check()` + config types | `traffic` (foundation) | This crate is only the backend behind the `QuotaBackend` seam |
| Key extraction + `on_backend_error` policy mapping | `transport` | Transport coupling stays out; this crate surfaces a `QuotaError` |

**The "do-not-depend-on" list:** never `tonic`/`http`. It depends *up* on `traffic` (for `QuotaBackend`) and on
`redis-storage`/`fred` for the Lua claim.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Backend I/O amortizes over `burst` requests/key/replica (no per-request hop) | `LeaseBook` | latency/load amplification |
| I2 | The atomic claim is a single-key Lua script (slot-safe) | `RedisClaimSource` | `CROSSSLOT` on a Redis Cluster |
| I3 | `LeaseBook` is pure and tested against an in-memory `ClaimSource` | crate structure | non-hermetic unit suite |
| I4 | `prune(lease_ms)` runs on a timer with `lease_ms` matching the profile window | caller (`transport`/loop) | unbounded memory or accounting drift |
| I5 | A claim failure is fail-soft (surfaced as `QuotaError`, not a panic) | `RedisLeaseBackend` | hot-path failure |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Hot path — a `distributed` check.** `RedisLeaseBackend` consults `LeaseBook`: if the key has remaining
local lease, serve it (no network). When the local lease is exhausted, call `ClaimSource` → `RedisClaimSource`
runs the single-key Lua script to claim `window_budget(rps, lease_ms)` tokens atomically; a fully-spent window
is cached locally so subsequent over-budget requests are rejected without touching Redis.

**Failure.** A claim error becomes `traffic::QuotaError`; `transport` maps it to the profile's
`on_backend_error` policy (degrade to the local governor, or reject). Requests served from an existing local
lease are unaffected by a Redis blip.

**Memory bounding.** `RedisLeaseBackend::prune(lease_ms)` evicts idle per-key leases on a timer; `tracked_keys()`
sizes the cadence. The `lease_ms` must equal the profiles' lease window or accounting drifts.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `traffic` | upstream | Separated Interface | `impl QuotaBackend` | distributed-mode enforcement |
| `redis-storage` / `fred` | upstream | Conformist | Lua `eval` claim script | the atomic claim |
| `transport` | downstream | Separated Interface (injected) | `with_traffic_backend(Arc<dyn QuotaBackend>)` | fleet-global limiting |

> **Stability seam:** the crate's public contract is `traffic::QuotaBackend` (implemented, not defined here) —
> the inversion is what lets `transport` wire it without `transport` knowing about Redis.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

N/A — no `tracing`/metrics of its own; the throttle metric is recorded by `transport`. Side effects: a
single-key Redis Lua `eval` per refill (not per request) and a local `dashmap` lease cache.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Lease-a-chunk (amortized) instead of check-per-request | [`README §Architecture`](../README.md) | Accepted |
| Pure `LeaseBook` behind a `ClaimSource` seam (hermetic unit suite) | [`README §Architecture`](../README.md) | Accepted |
| Single-key Lua claim for cluster-slot-safety | [`README §Architecture`](../README.md) | Accepted |
| Fail-soft via the profile's `on_backend_error` policy | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — a leased-budget limiter backend; leverage is the amortization that keeps the
  hot path off the network.
- **Stability:** evolving — this is `traffic` Step 2; `distributed` enforcement depends on this crate being
  wired.
- **Volatility:** low — the lease algorithm is settled; growth is operational (prune cadence, observability).
- **Deferred capabilities:** richer degradation policies and per-key telemetry; today the limit/decision shape
  is inherited from `traffic`.
