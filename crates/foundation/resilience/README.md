# `resilience` — Tower middleware for cascading-failure protection (circuit breaker · retry · timeout)

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — pure Tower middleware mechanism (egress fault tolerance) |
> | **Package** | `resilience` (dir: `crates/foundation/resilience`) |
> | **Consumed by** | `transport` (gRPC/Kafka clients), the `cqrs` bus; config via `infra-config` |
> | **Depends on** | `tower`, `arc-swap`, `tokio`, `thiserror`, `rand`, `serde` (optional), `error` |
> | **Stability** | stable contract (production-ready, no stubs) |
> | **Feature flags** | `serde` (off by default — adds wire types) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`resilience` provides production-grade Tower middleware layers protecting microservices against
cascading failures at the **outbound** transport boundary: a **circuit breaker** (fail fast when a
downstream is unhealthy), **retry** (transient failures with exponential backoff + jitter), and
**timeout** (an absolute per-request deadline). It sits between the `transport` clients and the `cqrs`
bus — every outbound call wraps through these layers, so resilience policy is fleet-wide without
touching business logic. It is the egress counterpart to [`traffic`](../traffic) (ingress).

**Architectural boundary** — a **pure library**: no file IO, no `notify`, no env, no spawned tasks.
The crate stays pure; its named **profiles** sit behind `Arc<ArcSwap<_>>` handles for lock-free
hot-reload, while parsing/validation/bindings/file-watching live in
[`infra-config`](../infra-config).

---

## 📐 Architecture & key decisions

```
Caller (CQRS bus / gRPC handler)
   ▼  TimeoutLayer        → ResilienceError::Timeout         (total request budget; outermost)
   ▼  CircuitBreakerLayer → ResilienceError::CircuitOpen     (counts ALL attempts incl. retries)
   ▼  RetryLayer          → ResilienceError::MaxRetriesExhausted (backoff between attempts)
   ▼  Inner service (tonic client / Kafka producer / HTTP)
```

Circuit breaker state machine: `Closed → Open` on `failure_threshold` consecutive failures; after
`open_duration`, `Open → HalfOpen`; `HalfOpen → Closed` on `success_threshold` successes, or back to
`Open` on a probe failure. `half_open_max_calls` caps concurrent probes.

- **Layer order is load-bearing** — Timeout *wraps* CircuitBreaker *wraps* Retry. The circuit must
  sit **outside** retry so it counts retries as attempts and trips on time; invert them and the
  circuit only ever sees the first call.
- **Config sampled once per `call`** — each operation `ArcSwap::load`s a single snapshot so it reasons
  against consistent values; config swaps are lock-free and **never reset live state** (counters,
  timers, circuit state).
- **`JitterKind::Full` default** — distributes retry delays over `[0, cap]`, so a fleet retrying the
  same downstream after an outage doesn't spike in lockstep (thundering-herd mitigation).
- **`serde` wire types bridge the generic boundary** — `RetryConfig<B>` is generic for zero-cost
  dispatch and can't be deserialized; non-generic `…Spec` types deserialize then `resolve()` into the
  monomorphized runtime types.

---

## 🔌 Public API & contract

```rust
pub enum ResilienceError<E> {       // thiserror::Error
    CircuitOpen,                    // downstream assumed down; request NOT forwarded
    Timeout(Duration),
    MaxRetriesExhausted(u32),
    Inner(E),                       // the ONLY variant carrying downstream error state
}

pub trait BackoffStrategy: Send + Sync + Clone + 'static { fn next_delay(&self, attempt: u32) -> Duration; } // attempt 1-indexed
pub struct ExponentialBackoff { pub base_ms: u64, pub max_ms: u64, pub jitter: JitterKind } // default 50ms / 10_000ms / Full
pub enum JitterKind { None, Full, Equal }   // exponent clamped to 30 to avoid u64 overflow

pub trait RetryPolicy<E>: Send + Sync + Clone + 'static { fn should_retry(&self, error: &E, attempt: u32) -> bool; }
// DefaultRetryPolicy (delegates to AppError::is_retryable) · AlwaysRetryPolicy · NeverRetryPolicy

// Layers — new(config) seeds a fresh ArcSwap; from_handle(...) shares one; handle() hands it back for control-plane store()
CircuitBreakerLayer::new(CircuitBreakerConfig) | ::from_handle(Arc<ArcSwap<_>>) | .handle()
RetryLayer::new(RetryConfig<B>, policy: P)
TimeoutLayer::new(TimeoutConfig) | ::from_handle(Arc<ArcSwap<_>>) | .handle()

// ResilienceProfile: bundles one timeout + CB + retry as a named class-of-service; timeout/CB behind shared ArcSwap.
impl ResilienceProfile { fn timeout_layer(&self); fn circuit_breaker_layer(&self); fn apply(&self, ResilienceProfileSpec) -> RetryConfig<ExponentialBackoff>; }
```

Config structs (defaults): `CircuitBreakerConfig { failure_threshold: 5, success_threshold: 2,
open_duration: 30s, half_open_max_calls: 1 }`, `RetryConfig { max_attempts: 3, backoff }`,
`TimeoutConfig { duration }`.

> **Contract notes:** `Inner(E)` is the only variant carrying downstream state; the rest are
> middleware-emitted. `CircuitBreakerService`/`TimeoutService` are `Clone` (clones share the same
> `Arc` state) — required by tonic, which clones the service per RPC. `RetryService` requires
> `S: Clone` **and** `Req: Clone` (it re-issues the request per attempt). With `serde` on, `Duration`
> fields serialize as flat ms integers (`open_duration` ⇄ `open_duration_ms`).

---

## 📦 Integration

```toml
[dependencies]
resilience = { workspace = true }   # add features = ["serde"] to parse config
```

```rust
use tower::ServiceBuilder;
use resilience::{circuit_breaker::*, retry::*, timeout::*};

// Order matters: Timeout (outer) → CircuitBreaker → Retry → inner.
let resilient = ServiceBuilder::new()
    .layer(TimeoutLayer::new(TimeoutConfig::from_secs(5)))
    .layer(CircuitBreakerLayer::new(CircuitBreakerConfig::new()
        .failure_threshold(5).open_duration(Duration::from_secs(30))
        .success_threshold(2).half_open_max_calls(1)))
    .layer(RetryLayer::new(RetryConfig::default_exponential(), DefaultRetryPolicy))
    .service(inner_grpc_client);   // inner must be cheaply Clone (Arc-backed or tower::Buffer)
```

Loading/validating profiles and resolving bindings (`"post-command" → "critical"`) lives in
[`infra-config`](../infra-config).

---

## ⚙️ Configuration & feature flags

Pure library — **no environment variables, no process**. Config is passed programmatically (static
use) or sourced externally and applied through `ResilienceProfile` handles (hot-reload via
`infra-config`).

**Feature flags:**
- `serde` — off by default; adds `Serialize`/`Deserialize` to config + wire types
  (`CircuitBreakerConfig`, `TimeoutConfig`, `JitterKind`, `BackoffSpec`, `RetrySpec`,
  `ResilienceProfileSpec`). Off ⇒ the crate links no serde code.

---

## 🔭 Observability

`tracing` events at state transitions: circuit transition (`INFO` `prev`/`next`), circuit tripped
(`WARN` `+failures`), probe failed (`WARN`), retry scheduled (`WARN` `attempt`/`max_attempts`/`delay_ms`),
request timeout (`WARN` `timeout_ms`). No OTel metric exports yet — add via the `telemetry` crate.

Suggested service-level alerts: `CircuitOpen` transition ⇒ critical; repeated HalfOpen→Open with no
recovery ⇒ critical; `MaxRetriesExhausted` rate ⇒ warn; `Timeout` rate > 1% ⇒ warn.

---

## 🧪 Testing

```bash
cargo test   -p resilience                 # unit tests, no external deps
cargo test   -p resilience --features serde # wire (de)serialization
cargo clippy -p resilience --all-targets
```

Pure in-process library — no Docker, DB, or broker. When changing the engine, preserve: config sampled
once per op (never re-load mid-decision), swaps never reset live state, and boxed `Send` futures must
not hold a non-`Send` value across `.await`.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. Circuit trips immediately on the first call.**
A previously-constructed `CircuitBreakerLayer` (which owns the `Arc<StateMachine>`) is being reused
with persisted state. Construct a **fresh** layer at startup; don't stash it in a `once_cell`/`static`
unless you explicitly want cross-restart state.

**2. Retries amplify load instead of reducing it (3–4× spike on a downstream outage).**
`JitterKind::None`/`Equal` across a fleet ⇒ lockstep retries. Use `JitterKind::Full` (the default).
Also verify `CircuitBreakerLayer` wraps `RetryLayer` — otherwise the circuit only counts the initial
call and trips too late.

**3. `Req: Clone` compile error on `RetryLayer`.**
`RetryService` clones the request to re-issue it per attempt. `prost`/tonic structs derive `Clone`, but
custom wrappers may not — derive `Clone`, or pass only the cloneable inner proto into the retry region.
