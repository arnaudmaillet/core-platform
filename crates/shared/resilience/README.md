# `resilience` — Tower Middleware for Cascading Failure Protection

## 🎯 Overview & Service Role

`resilience` is a **pure library crate** that provides production-grade Tower middleware layers for protecting microservices against cascading failures at the transport boundary. It implements three composable fault-tolerance primitives:

- **Circuit Breaker** — stops calling a failing downstream entirely, fails fast until the dependency recovers.
- **Retry** — retries transient failures with exponential backoff and optional jitter, decoupled from the error classification logic.
- **Timeout** — enforces an absolute per-request deadline, preventing slow dependencies from exhausting upstream goroutines/tasks.

In addition to the runtime mechanism, the crate exposes an **externalized-config boundary**: optional `serde`-derived wire types and named **profiles** whose live values sit behind `Arc<ArcSwap<_>>` handles for **lock-free hot-reload**. The crate itself stays pure (no file IO, no `notify`); the [`resilience-config`](../resilience-config) companion crate owns parsing, validation, fleet bindings, and the file watcher.

**Critical role in the ecosystem:** this crate sits between the `transport` crate (gRPC/Kafka clients) and the `cqrs` bus. Any outbound call to a downstream service wraps through these layers — enabling fleet-wide resilience policy without modifying business logic.

> **Implementation status:** Production-ready. The full runtime engine — `StateMachine` transitions, `CircuitBreakerService`, `RetryService`, and `TimeoutService` — is implemented and covered by unit tests, alongside the `serde` boundary, the `ArcSwap` hot-reload plumbing, and the `ResilienceProfile` resolution layer. No `todo!()` stubs remain.

---

## 📐 Architecture & Concepts

### Layer Stack (outermost → innermost)

```
┌─────────────────────────────────────────────────────────────────┐
│  Caller (CQRS Bus / gRPC Handler)                               │
└────────────────────────┬────────────────────────────────────────┘
                         │  tower::Service<Req>
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│  TimeoutLayer              [ResilienceError::Timeout]           │
│  Enforces total request budget — wraps everything below.        │
└────────────────────────┬────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│  CircuitBreakerLayer       [ResilienceError::CircuitOpen]       │
│  Counts all attempts (including retries). Trips on consecutive  │
│  failures; admits probe calls after open_duration elapses.      │
└────────────────────────┬────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│  RetryLayer                [ResilienceError::MaxRetriesExhausted│
│  Retries transient errors. Consults RetryPolicy per attempt.    │
│  Waits BackoffStrategy::next_delay(attempt) between attempts.   │
└────────────────────────┬────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────────────────┐
│  Inner Service (S: tower::Service)                              │
│  e.g. tonic gRPC client, Kafka producer, HTTP client           │
└─────────────────────────────────────────────────────────────────┘
```

### Circuit Breaker State Machine

```
                  failures >= failure_threshold
    ┌──────────┐ ──────────────────────────────► ┌──────────┐
    │  Closed  │                                  │   Open   │
    │(nominal) │ ◄────────────────────────────── │(fast-fail│
    └──────────┘  successes >= success_threshold  └────┬─────┘
                  (HalfOpen → Closed)                  │
                                                       │ open_duration elapsed
                                                       ▼
                                                  ┌──────────┐
                                                  │ HalfOpen │  ◄── probe fails → Open
                                                  │ (probing)│
                                                  └──────────┘
                                  max concurrent probes: half_open_max_calls
```

### Resilience Guarantees & High-Load Behavior

| Primitive | Backpressure Mechanism | Memory Footprint | Thread Safety |
|---|---|---|---|
| **Circuit Breaker** | Rejects immediately when Open; no goroutine/task leak | `Arc<Mutex<Inner>>` (runtime state) + `Arc<ArcSwap<CircuitBreakerConfig>>` (hot-swappable config) per dependency | State transitions atomic under `tokio::sync::Mutex`; config swaps are lock-free and never disturb live state |
| **Retry** | Bounded by `max_attempts`; sleeps via `tokio::time::sleep` (no thread blocking) | No heap allocation between attempts beyond the boxed future | Stateless — `policy` and `config` are `Clone`d per `call` |
| **Timeout** | Cancels the inner future via `tokio::time::timeout` — no resource leak if inner fut is cancel-safe | `Arc<ArcSwap<TimeoutConfig>>` — one snapshot loaded per `call` | Lock-free reads; deadline captured once at the start of `call()` for request-scoped consistency |

**Thundering-herd mitigation:** `JitterKind::Full` (the default) distributes retry delays uniformly over `[0, cap]`. Across a fleet of N services retrying the same dependency, aggregate call rate remains bounded instead of spiking in lockstep after an outage.

**Half-Open concurrency control:** `half_open_max_calls` (default `1`) is a hard cap on in-flight probe calls. Excess probe attempts are rejected with `CircuitOpen` without touching the downstream, preventing the recovery window from being flooded.

---

## 🔌 Public Interfaces & API Contract

### `ResilienceError<E>` — unified error envelope

```rust
// src/error.rs
pub enum ResilienceError<E> {
    /// Circuit is Open — downstream is assumed unavailable; request was not forwarded.
    CircuitOpen,
    /// Inner service did not respond within the configured deadline.
    Timeout(Duration),
    /// All retry attempts exhausted; contains the configured max attempt count.
    MaxRetriesExhausted(u32),
    /// Non-retryable error propagated from the inner service.
    Inner(E),
}
```

**Invariants:**
- `Inner(E)` is the only variant that carries the downstream's error. The other variants are middleware-emitted and contain no downstream state.
- `ResilienceError<E>` implements `std::error::Error` via `thiserror::Error`.

---

### `BackoffStrategy` — injectable delay strategy

```rust
// src/retry/backoff/strategy.rs
pub trait BackoffStrategy: Send + Sync + Clone + 'static {
    /// `attempt` is 1-indexed: first retry receives `attempt = 1`.
    fn next_delay(&self, attempt: u32) -> Duration;
}
```

**Built-in implementation — `ExponentialBackoff`:**

```rust
pub struct ExponentialBackoff {
    pub base_ms: u64,       // delay for attempt=1 before jitter (default: 50ms)
    pub max_ms:  u64,       // hard cap on computed delay      (default: 10_000ms)
    pub jitter:  JitterKind,
}

pub enum JitterKind {
    None,   // deterministic: min(base * 2^(attempt-1), max)
    Full,   // rand(0, cap)           — recommended; eliminates thundering herd
    Equal,  // cap/2 + rand(0, cap/2) — guarantees ≥50% of cap as minimum wait
}
```

Exponent is clamped to 30 before shifting to prevent `u64` overflow on pathological attempt counts.

---

### `RetryPolicy<E>` — error classification for retry

```rust
// src/retry/policy.rs
pub trait RetryPolicy<E>: Send + Sync + Clone + 'static {
    /// `attempt` is 1-indexed. Return `true` to schedule another attempt.
    fn should_retry(&self, error: &E, attempt: u32) -> bool;
}
```

| Implementation | Behaviour | Use-case |
|---|---|---|
| `DefaultRetryPolicy` | Delegates to `AppError::is_retryable()` from `error` crate | Any service error that implements `AppError` |
| `AlwaysRetryPolicy` | Always returns `true` | Third-party errors that do not implement `AppError` |
| `NeverRetryPolicy` | Always returns `false` | Tests, or disabling retry without removing the layer |

---

### Tower Layers — construction API

```rust
// Circuit Breaker — src/circuit_breaker/layer.rs
CircuitBreakerLayer::new(config: CircuitBreakerConfig) -> CircuitBreakerLayer
CircuitBreakerLayer::from_handle(Arc<ArcSwap<CircuitBreakerConfig>>) -> CircuitBreakerLayer
CircuitBreakerLayer::handle(&self) -> Arc<ArcSwap<CircuitBreakerConfig>>  // for control-plane store()

// Retry — src/retry/layer.rs
RetryLayer::new(config: RetryConfig<B>, policy: P) -> RetryLayer<P, B>

// Timeout — src/timeout/layer.rs
TimeoutLayer::new(config: TimeoutConfig) -> TimeoutLayer
TimeoutLayer::from_handle(Arc<ArcSwap<TimeoutConfig>>) -> TimeoutLayer
TimeoutLayer::handle(&self) -> Arc<ArcSwap<TimeoutConfig>>               // for control-plane store()
```

`new(config)` allocates a fresh `ArcSwap` seeded with `config` (static use). `from_handle(...)` shares an externally-owned handle — this is how [`ResilienceProfile`](#hot-reload--profiles) binds the same hot-swappable config into multiple layers. `handle()` hands the shared `ArcSwap` back out so a watcher can `store()` new values at runtime.

`CircuitBreakerService` and `TimeoutService` are `Clone` (clones share the same `Arc` state/config handle) — required by generated clients such as tonic that clone the service per RPC.

---

### Configuration structs

```rust
// src/circuit_breaker/config.rs
CircuitBreakerConfig {
    failure_threshold:    u32,      // Closed → Open  (default: 5)
    success_threshold:    u32,      // HalfOpen → Closed (default: 2)
    open_duration:        Duration, // how long the circuit stays Open (default: 30s)
    half_open_max_calls:  u32,      // concurrent probe slots in HalfOpen (default: 1)
}

// src/retry/config.rs
RetryConfig<B: BackoffStrategy> {
    max_attempts: u32,  // retry count, excluding the initial attempt (default: 3)
    backoff:      B,
}
// Shortcut: RetryConfig::default_exponential() → 3 retries, ExponentialBackoff::default()

// src/timeout/config.rs
TimeoutConfig { duration: Duration }
// Constructors: TimeoutConfig::from_secs(u64) | TimeoutConfig::from_millis(u64)
```

With the `serde` feature on, `CircuitBreakerConfig`, `TimeoutConfig`, and `JitterKind` derive `Serialize`/`Deserialize`. `Duration` fields serialize as flat millisecond integers (`open_duration` ⇄ `open_duration_ms`, `duration` ⇄ `duration_ms`) so they read cleanly from TOML.

---

### Wire types — `serde` boundary (feature `serde`)

`RetryConfig<B>` is generic over `BackoffStrategy` for zero-cost dispatch, so it can't be deserialized directly. Non-generic **spec** types bridge the gap: they deserialize from config, then `resolve()` into the concrete, monomorphized runtime types. Serialization never touches the trait boundary.

```rust
// src/retry/backoff/spec.rs
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackoffSpec {
    Exponential { base_ms: u64, max_ms: u64, jitter: JitterKind },
}
impl BackoffSpec { pub fn resolve(self) -> ExponentialBackoff; }

// src/retry/config.rs
pub struct RetrySpec { pub max_attempts: u32, pub backoff: BackoffSpec }
impl RetrySpec { pub fn resolve(self) -> RetryConfig<ExponentialBackoff>; }
```

```toml
backoff = { kind = "exponential", base_ms = 50, max_ms = 10_000, jitter = "full" }
```

---

### Hot-Reload & Profiles

A **`ResilienceProfile`** bundles one timeout + circuit breaker + retry policy into a named class-of-service (`"standard"`, `"critical"`, `"aggressive"`, …). Its timeout and circuit-breaker values live behind shared `Arc<ArcSwap<_>>` handles, so a control plane can swap them at runtime — lock-free, with in-flight requests keeping the snapshot they captured at `call()`.

```rust
// src/profile.rs
pub struct ResilienceProfileSpec {            // wire form (serde): deserialized from config
    pub timeout: TimeoutConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub retry: RetrySpec,
}
impl ResilienceProfileSpec { pub fn resolve(self) -> ResilienceProfile; }

pub struct ResilienceProfile {                // runtime form: shared hot-reload handles
    pub timeout: Arc<ArcSwap<TimeoutConfig>>,
    pub circuit_breaker: Arc<ArcSwap<CircuitBreakerConfig>>,
    pub retry: RetryConfig<ExponentialBackoff>,
}
impl ResilienceProfile {
    pub fn timeout_layer(&self) -> TimeoutLayer;                 // bound to the shared handle
    pub fn circuit_breaker_layer(&self) -> CircuitBreakerLayer;  // bound to the shared handle
    pub fn apply(&self, spec: ResilienceProfileSpec) -> RetryConfig<ExponentialBackoff>;  // hot-swap
}
```

**Scope note:** timeout + circuit-breaker hot-reload (the incident-critical levers) is wired; retry is resolved into the profile but not behind `ArcSwap` (the retry layer is generic over the backoff strategy). `apply()` returns the new retry config for callers that rebuild the retry layer.

Loading profiles from `infrastructure.toml`, validating them, resolving fleet bindings (`"post-command" → "critical"`), and watching the file for changes all live in the [`resilience-config`](../resilience-config) crate.

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# In your service's Cargo.toml
[dependencies]
resilience = { workspace = true }
```

### Standard Bootstrap Pattern

```rust
use std::time::Duration;
use tower::ServiceBuilder;
use resilience::{
    circuit_breaker::{CircuitBreakerLayer, CircuitBreakerConfig},
    retry::{RetryLayer, RetryConfig, DefaultRetryPolicy},
    timeout::{TimeoutLayer, TimeoutConfig},
};

// Wrap any tower::Service with the full resilience stack.
// Order matters: Timeout (outermost) → CircuitBreaker → Retry → inner.
let resilient_svc = ServiceBuilder::new()
    .layer(TimeoutLayer::new(TimeoutConfig::from_secs(5)))
    .layer(CircuitBreakerLayer::new(
        CircuitBreakerConfig::new()
            .failure_threshold(5)
            .open_duration(Duration::from_secs(30))
            .success_threshold(2)
            .half_open_max_calls(1),
    ))
    .layer(RetryLayer::new(
        RetryConfig::default_exponential(), // 3 retries, 50ms–10s, full jitter
        DefaultRetryPolicy,                 // uses AppError::is_retryable()
    ))
    .service(inner_grpc_client);
```

**`S: Clone` requirement:** `RetryService` and `CircuitBreakerService` clone the inner service once per `call` invocation. The inner service must be cheaply cloneable — use an `Arc`-backed gRPC client or wrap with `tower::Buffer` for services that are not `Clone`.

**`Req: Clone` requirement:** `RetryService` re-issues the request on each attempt. The request type must implement `Clone`.

### Applying a single layer (lightweight use case)

```rust
// Timeout-only for a non-retryable one-shot call
use tower::ServiceBuilder;
use resilience::timeout::{TimeoutLayer, TimeoutConfig};

let svc = ServiceBuilder::new()
    .layer(TimeoutLayer::new(TimeoutConfig::from_millis(500)))
    .service(inner);
```

### Custom retry policy

```rust
use resilience::retry::RetryPolicy;

#[derive(Clone)]
struct GrpcRetryPolicy;

impl<E: std::fmt::Debug> RetryPolicy<E> for GrpcRetryPolicy {
    fn should_retry(&self, _error: &E, attempt: u32) -> bool {
        // Only retry the first two failures; give up after that.
        attempt <= 2
    }
}
```

---

## ⚙️ Configuration & Runtime Environment

This crate is a **pure library** — it consumes no environment variables and has no runtime process. Configuration is passed programmatically via the config structs (static use), or sourced externally and applied through `ResilienceProfile` handles when paired with [`resilience-config`](../resilience-config) (hot-reload). No file IO or `notify` dependency lives here.

### Compile-time / Cargo features

| Feature | Status | Description |
|---|---|---|
| `serde` | optional, off by default | Adds `Serialize`/`Deserialize` to the config + wire types (`CircuitBreakerConfig`, `TimeoutConfig`, `JitterKind`, `BackoffSpec`, `RetrySpec`, `ResilienceProfileSpec`). With it off, the crate links no serde code. Enable via `resilience = { workspace = true, features = ["serde"] }`. |

### Default production values summary

| Parameter | Default | Rationale |
|---|---|---|
| `CircuitBreakerConfig::failure_threshold` | `5` | Conservative; avoids tripping on isolated transient errors |
| `CircuitBreakerConfig::open_duration` | `30s` | Gives downstream time to recover before probe |
| `CircuitBreakerConfig::success_threshold` | `2` | Two consecutive successes confirm recovery |
| `CircuitBreakerConfig::half_open_max_calls` | `1` | Prevents probe flood during recovery |
| `RetryConfig::max_attempts` | `3` | 4 total attempts; balances latency vs. reliability |
| `ExponentialBackoff::base_ms` | `50ms` | Low first-retry latency for fast transients |
| `ExponentialBackoff::max_ms` | `10_000ms` | 10s cap; prevents indefinite back-pressure amplification |
| `ExponentialBackoff::jitter` | `Full` | Maximally spreads retry load across the fleet |

---

## 📈 Telemetry, Performance & Metrics

### Runtime prerequisites

- **Tokio runtime** — all async operations (`tokio::sync::Mutex`, `tokio::time::sleep`, `tokio::time::timeout`) require a Tokio multi-thread or current-thread runtime. The crate does not spawn tasks.
- **No CPU architecture constraints.** `rand` uses the platform CSPRNG.

### Structured log events (via `tracing`)

The following events are emitted at key state transitions:

| Event | Level | Fields | Trigger |
|---|---|---|---|
| Circuit state transition | `INFO` | `prev`, `next` | Closed → Open, HalfOpen → Closed |
| Circuit tripped | `WARN` | `prev`, `next`, `failures` | Closed → Open on failure threshold |
| Probe failed | `WARN` | `prev`, `next` | HalfOpen → Open on probe failure |
| Retry scheduled | `WARN` | `attempt`, `max_attempts`, `delay_ms` | Each retry before sleeping |
| Request timeout | `WARN` | `timeout_ms` | Inner future exceeds deadline |

### Prometheus / OTel metrics

<!-- TODO: [No OTel metric exports are defined in this crate yet. Add counters/histograms via the `telemetry` workspace crate.] -->

**Recommended alerts to implement at the service level:**

| Alert | Condition | Severity |
|---|---|---|
| `circuit_open` | Circuit transitions to Open | Critical |
| `retry_exhausted_rate` | `MaxRetriesExhausted` rate > threshold | Warning |
| `timeout_rate` | `Timeout` error rate > 1% of requests | Warning |
| `half_open_probe_failure` | Repeated HalfOpen → Open cycles with no recovery | Critical |

---

## 🛠️ Local Development & Contribution

### Build & lint

```bash
# From the workspace root
cargo build -p resilience

# Clippy (workspace lint rules apply)
cargo clippy -p resilience -- -D warnings

# Format
cargo fmt -p resilience
```

### Run tests

```bash
# Unit tests (no external dependencies required)
cargo test -p resilience

# With tokio test-util (already in dev-dependencies)
cargo test -p resilience -- --nocapture
```

### No external service dependencies

This crate is a pure in-process library. **No Docker Compose, no database, no broker** is required for local development or testing.

### Modifying the runtime engine

The `StateMachine` transitions and the three service `call` bodies are fully implemented and unit-tested. When changing state-machine invariants or a `call` path, preserve these rules:

- **Config is sampled once per operation** (`ArcSwap::load_full` / `load`) so a single call reasons against a consistent snapshot — never re-load mid-decision.
- **Config swaps must never reset live state** (counters, timers, circuit state).
- **Boxed `Send` futures** must not hold a non-`Send` response/error across an `.await` (see `RetryService`, which resolves each attempt to a `Send` delay before sleeping).

After changing anything, ensure:

1. `cargo test -p resilience --features serde` passes (add a `#[tokio::test]` in the same file for any new transition).
2. `cargo clippy -p resilience --all-targets -- -D warnings` is clean.

---

## 🚨 Troubleshooting & Runbook

### 1. Circuit trips immediately on first call

**Symptom:** `ResilienceError::CircuitOpen` on the very first request.

**Root cause:** A previously-constructed `CircuitBreakerLayer` is being reused across application restarts without re-initializing. Because `CircuitBreakerLayer` owns the `Arc<StateMachine>`, its state persists for the lifetime of the instance.

**Mitigation:** Always construct a fresh `CircuitBreakerLayer` during application startup. Do not store the layer in a global static with `once_cell` / `lazy_static` unless you explicitly want persistent cross-request state.

---

### 2. Retries amplify load instead of reducing it

**Symptom:** A dependency outage causes a 3–4× spike in inbound call rate to that dependency.

**Root cause:** `JitterKind::None` or `JitterKind::Equal` in use across a large fleet — all instances retry at near-identical intervals.

**Mitigation:** Switch to `JitterKind::Full` (the default in `ExponentialBackoff::default()`). Confirm with:

```rust
let backoff = ExponentialBackoff::new(50, 10_000, JitterKind::Full);
```

Also verify that `CircuitBreakerLayer` sits **outside** (wrapping) `RetryLayer` in your `ServiceBuilder` chain — otherwise the circuit counts only the initial call, not the retries, and trips later than intended.

---

### 3. `Req: Clone` compile error on `RetryLayer`

**Symptom:**

```
error[E0277]: the trait bound `MyRequest: Clone` is not satisfied
  --> src/grpc/client.rs:42:10
   |
42 |     .layer(RetryLayer::new(...))
```

**Root cause:** `RetryService` must clone the request to re-issue it on each attempt. Request types generated by `prost` (tonic proto structs) derive `Clone` by default, but custom wrapper types may not.

**Mitigation:** Derive or implement `Clone` on the request type, or wrap the retryable region so only the inner cloneable proto is passed to the retry layer.
