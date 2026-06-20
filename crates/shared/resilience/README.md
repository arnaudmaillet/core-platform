# `resilience` — Interface Contract

## 🎯 Responsibility

Provide reusable Tower middleware layers (Circuit Breaker, Retry, Timeout) that protect microservices against cascading failures, with zero business logic and no inbound consumers.

---

## 🔌 Public Interfaces (Traits & API)

### `BackoffStrategy` — injectable delay strategy for retry

```rust
// src/retry/backoff/strategy.rs
pub trait BackoffStrategy: Send + Sync + Clone + 'static {
    /// `attempt` is 1-indexed: the first retry receives `attempt = 1`.
    fn next_delay(&self, attempt: u32) -> Duration;
}
```

Built-in implementation: `ExponentialBackoff { base_ms, max_ms, jitter: JitterKind }`.
`JitterKind` variants: `None` | `Full` (default, thundering-herd safe) | `Equal`.

---

### `RetryPolicy<E>` — decides whether a failed attempt should be retried

```rust
// src/retry/policy.rs
pub trait RetryPolicy<E>: Send + Sync + Clone + 'static {
    fn should_retry(&self, error: &E, attempt: u32) -> bool;
}
```

Built-in implementations:

| Type | Behaviour |
|---|---|
| `DefaultRetryPolicy` | Delegates to `AppError::is_retryable()` from the `error` crate |
| `AlwaysRetryPolicy` | Retries unconditionally (useful for non-`AppError` third-party errors) |
| `NeverRetryPolicy` | Never retries (no-op / test double) |

---

### `ResilienceError<E>` — unified error envelope

```rust
// src/error.rs
pub enum ResilienceError<E> {
    CircuitOpen,                    // request rejected — circuit is open
    Timeout(Duration),              // inner service exceeded the deadline
    MaxRetriesExhausted(u32),       // all retry attempts failed
    Inner(E),                       // non-retryable error from the inner service
}
```

---

### Tower Layers — the three middleware entry points

```rust
// Circuit Breaker — src/circuit_breaker/layer.rs
CircuitBreakerLayer::new(CircuitBreakerConfig::default())

// Retry — src/retry/layer.rs
RetryLayer::new(RetryConfig::default_exponential(), DefaultRetryPolicy)

// Timeout — src/timeout/layer.rs
TimeoutLayer::new(TimeoutConfig::from_secs(5))
```

**Recommended composition order** (outermost → innermost):

```rust
ServiceBuilder::new()
    .layer(TimeoutLayer::new(TimeoutConfig::from_secs(5)))
    .layer(CircuitBreakerLayer::new(CircuitBreakerConfig::default()))
    .layer(RetryLayer::new(RetryConfig::default_exponential(), DefaultRetryPolicy))
    .service(inner_service)
```

> Timeout wraps everything so the total budget is enforced. The circuit breaker sits outside retry so it counts all attempts, not just the first call.

---

### Configuration structs

```rust
// src/circuit_breaker/config.rs
CircuitBreakerConfig {
    failure_threshold: u32,      // default: 5  — consecutive failures to trip (Closed → Open)
    success_threshold: u32,      // default: 2  — consecutive successes to reset (HalfOpen → Closed)
    open_duration: Duration,     // default: 30s — how long the circuit stays open
    half_open_max_calls: u32,    // default: 1  — concurrent probe calls in HalfOpen
}

// src/retry/config.rs
RetryConfig<B: BackoffStrategy> {
    max_attempts: u32,           // default: 3  — retries, not counting the initial call
    backoff: B,
}

// src/timeout/config.rs
TimeoutConfig {
    duration: Duration,
}
// Constructors: TimeoutConfig::from_secs(u64) | TimeoutConfig::from_millis(u64)
```

---

## 📦 Entry Points & Consumption

**This crate consumes nothing** — no Kafka, no gRPC, no inbound HTTP. It is a pure library.

**Runtime injection required:** none. All state is owned by the layer instances.

**To add it to a service:**

```toml
# service Cargo.toml
resilience = { workspace = true }
```

**`S: Clone` requirement:** `RetryService` and `CircuitBreakerService` clone the inner service once per `call` — the inner service must be cheaply cloneable (e.g. an `Arc`-backed gRPC client or a `tower::Buffer`-wrapped service).

**`AppError` integration:** `DefaultRetryPolicy` requires `E: AppError` (from the `error` crate). If the inner service error does not implement `AppError`, use `AlwaysRetryPolicy` or provide a custom `RetryPolicy` impl.

---

## 📝 Key Files

| File | What to read |
|---|---|
| `src/error.rs` | `ResilienceError<E>` — the only error type exposed to callers |
| `src/circuit_breaker/layer.rs` + `src/circuit_breaker/state.rs` | Tower glue and the state machine contract (`state()`, `on_success()`, `on_failure()`) |
| `src/retry/policy.rs` + `src/retry/backoff/strategy.rs` | The two injectable traits that control retry behaviour |
