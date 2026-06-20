# `error` — Shared Error Infrastructure for the Platform

## 🎯 Overview & Service Role

`error` is the workspace-wide foundation for structured, observable, distributed error handling. It defines **no business logic and no domain** — it provides the contract, vocabulary, and serialization primitives that let every microservice define its own error enum independently while guaranteeing a uniform, client-safe, telemetry-rich output at the platform boundary.

**Core objectives:**

- **Uniformity across services:** One trait, one JSON shape, one severity vocabulary — regardless of which microservice raises an error.
- **Zero information leakage:** Internal identifiers (`trace_id`, `span_id`) are captured for logs and never sent to API clients.
- **Type preservation end-to-end:** `DistributedError<E>` keeps the concrete error type; no `Box<dyn Error>` erasure, full pattern-matching preserved.
- **Progressive disclosure:** Only two methods are mandatory on `AppError`; all others have conservative production-safe defaults.

**Consumed by:** `cqrs`, `transport`, `resilience`, and every service crate in the workspace.

---

## 📐 Architecture & Concepts

### Component Map

```
┌──────────────────────────────────────────────────────────────────────┐
│                        error  crate                                  │
│                                                                      │
│  ┌────────────────────┐   implements   ┌───────────────────────────┐ │
│  │  Service error enum│ ─────────────► │  AppError  (traits.rs)    │ │
│  │  (e.g. AuthError)  │               │  + IntoApiResponse        │ │
│  └────────────────────┘               │    (blanket impl)         │ │
│           │                           └───────────┬───────────────┘ │
│           │ wrapped by                            │                  │
│           ▼                                       │                  │
│  ┌────────────────────────────────┐               │                  │
│  │  DistributedError<E>           │               │                  │
│  │  (context.rs)                  │◄──────────────┘                  │
│  │                                │                                  │
│  │  • error: E          (typed)   │  .log() ──► tracing event        │
│  │  • context: ErrorContext       │             (trace/span inside)  │
│  └────────────┬───────────────────┘                                  │
│               │                                                      │
│               │ into_api_response()                                  │
│               ▼                                                      │
│  ┌────────────────────────────────┐  JSON  ┌───────────────────────┐ │
│  │  ApiErrorResponse  (http.rs)   │ ──────►│  API Client           │ │
│  │  (NO trace_id / span_id)       │        └───────────────────────┘ │
│  └────────────────────────────────┘                                  │
└──────────────────────────────────────────────────────────────────────┘
```

### The Four Pillars

| Pillar | Type | Responsibility |
|---|---|---|
| **Contract** | `AppError` + `IntoApiResponse` | Trait each service error enum implements |
| **Vocabulary** | `Severity` | Unified urgency ranking driving paging and log levels |
| **Context** | `ErrorContext` + `DistributedError<E>` | Request/trace metadata envelope with structured logging |
| **Wire Format** | `ApiErrorResponse` + `into_api_response` | The only JSON shape that ever reaches a client |

### Resilience Guarantees & High-Load Behavior

- **Stateless library:** No internal state, no allocations at startup, no background goroutines. Zero overhead for services that never raise an error.
- **No heap allocation on the hot path:** `AppError` methods return `&'static str`; `ErrorContext` allocates only `HashMap` entries you explicitly add. The envelope itself is stack-allocated until the service returns it.
- **Backpressure:** Not applicable — this crate never buffers. All decisions (how many errors to log, whether to sample) are delegated to the `tracing` subscriber installed by the consuming service.
- **No external dependency failure:** The crate has no network I/O. It cannot be the cause of a cascading failure.
- **`DistributedError` size on hot error paths:** The envelope carries a full `ErrorContext` (including `HashMap`). On latency-sensitive hot paths, services may wrap it in a `Box<DistributedError<E>>` to keep the `Result` size small (see `#[allow(clippy::result_large_err)]` in the example).

---

## 🔌 Public Interfaces & API Contract

### `AppError` — service error contract

```rust
pub trait AppError: std::error::Error + Send + Sync + 'static {
    // ── Required ───────────────────────────────────────────────────────────
    fn error_code(&self) -> &'static str;   // "AUTH_TOKEN_EXPIRED" — breaking change if renamed
    fn http_status(&self) -> StatusCode;

    // ── Optional (production-safe defaults) ───────────────────────────────
    fn severity(&self) -> Severity          { Severity::Medium }
    fn is_retryable(&self) -> bool          { false }
    fn category(&self) -> &'static str      { "UNKNOWN" }
    fn user_facing_message(&self) -> &'static str { "An error occurred." }
}
```

> **Stability contract:** `error_code` is part of the public API surface. Clients and dashboards key off it — treat any rename as a breaking change requiring a versioned migration.

### `IntoApiResponse` — blanket, zero-boilerplate conversion

```rust
pub trait IntoApiResponse: AppError + Sized {
    fn to_api_response(&self, ctx: &ErrorContext) -> ApiErrorResponse;
}

// Automatically implemented for every AppError — do NOT override.
impl<E: AppError> IntoApiResponse for E {}
```

### `Severity` — unified urgency vocabulary

| Variant | `log_level()` | `should_page()` | Intended use |
|---|---|---|---|
| `Critical` | `ERROR` | `true` | Service down, data integrity at risk |
| `High` | `ERROR` | `true` | Significant degradation impacting users |
| `Medium` | `WARN` | `false` | Recoverable or partial failure (default) |
| `Low` | `INFO` | `false` | Expected under normal operation (e.g. validation) |
| `Info` | `DEBUG` | `false` | Purely informational |

> `Severity` implements `Ord`: `Critical < High < Medium < Low < Info`. "Higher urgency = lower value."

### `ErrorContext` — distributed request context builder

```rust
pub struct ErrorContext {
    pub request_id:   Uuid,             // auto-generated by ::new()
    pub trace_id:     Option<String>,   // OTel / Jaeger — stays in logs only
    pub span_id:      Option<String>,   // stays in logs only
    pub service_name: &'static str,
    pub timestamp:    DateTime<Utc>,
    pub metadata:     HashMap<String, String>,  // surfaced as `details` in client JSON
}

// Builder API
ErrorContext::new("my-service")
    .with_trace("4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7")
    .with_meta("user_id", "u_42")
    .with_meta("route", "POST /v1/sessions");
```

### `DistributedError<E>` — type-safe error envelope

```rust
pub struct DistributedError<E: AppError> {
    pub error:   E,
    pub context: ErrorContext,
}

impl<E: AppError> DistributedError<E> {
    pub fn new(error: E, context: ErrorContext) -> Self;

    /// Emits a structured tracing event.
    /// Level = error.severity().log_level()
    /// Fields: request_id, trace_id, span_id, service, severity,
    ///         error_code, category, retryable, error.message
    pub fn log(&self);
}
```

### `ApiErrorResponse` — the only JSON payload sent to clients

```rust
pub struct ApiErrorResponse {
    pub error_code:  String,            // from AppError::error_code()
    pub message:     String,            // from AppError::user_facing_message()
    pub request_id:  Uuid,              // safe to expose; user quotes to support
    pub service:     String,
    pub severity:    Severity,
    pub retryable:   bool,
    pub category:    String,
    pub timestamp:   DateTime<Utc>,     // RFC 3339 on the wire
    pub details:     HashMap<String, String>,  // from ErrorContext::metadata
    // trace_id and span_id are intentionally absent
}
```

**Wire example:**

```json
{
  "error_code": "AUTH_TOKEN_EXPIRED",
  "message": "Your session has expired, please sign in again.",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "service": "auth-service",
  "severity": "Low",
  "retryable": false,
  "category": "AUTH",
  "timestamp": "2024-01-15T10:30:00Z",
  "details": { "route": "POST /v1/sessions" }
}
```

### `into_api_response` — free conversion function

```rust
pub fn into_api_response<E: AppError>(err: &DistributedError<E>) -> ApiErrorResponse
```

Pair the return value with `err.error.http_status()` in your framework's response type.

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# service/Cargo.toml
[dependencies]
error = { workspace = true }
thiserror = { workspace = true }  # recommended for ergonomic error definitions
```

### Standard bootstrap pattern (axum)

The orphan rule forbids `impl IntoResponse for DistributedError<MyError>` — both `IntoResponse` and `DistributedError` are foreign types. The idiomatic solution is a **one-line service-owned newtype**:

```rust
use axum::{Json, response::{IntoResponse, Response}};
use error::{AppError, DistributedError, ErrorContext, Severity, into_api_response};
use http::StatusCode;
use thiserror::Error;

// Step 1 — Define the domain error enum.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("the provided session token has expired")]
    TokenExpired,
    #[error("identity provider is temporarily unavailable")]
    UpstreamUnavailable,
}

// Step 2 — Implement the shared contract.
impl AppError for AuthError {
    fn error_code(&self) -> &'static str {
        match self {
            AuthError::TokenExpired        => "AUTH_TOKEN_EXPIRED",
            AuthError::UpstreamUnavailable => "AUTH_UPSTREAM_UNAVAILABLE",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            AuthError::TokenExpired        => StatusCode::UNAUTHORIZED,
            AuthError::UpstreamUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            AuthError::TokenExpired        => Severity::Low,
            AuthError::UpstreamUnavailable => Severity::High,
        }
    }

    fn is_retryable(&self) -> bool { matches!(self, AuthError::UpstreamUnavailable) }
    fn category(&self) -> &'static str { "AUTH" }
}

// Step 3 — Newtype to satisfy the orphan rule (axum only).
pub struct ApiError(pub DistributedError<AuthError>);

impl From<DistributedError<AuthError>> for ApiError {
    fn from(e: DistributedError<AuthError>) -> Self { ApiError(e) }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        err.log();                              // structured log with trace/span ids
        let status = err.error.http_status();
        let body = into_api_response(&err);     // safe client JSON, no internal fields
        (status, Json(body)).into_response()
    }
}

// Step 4 — Use in a handler. `?` converts via `From` automatically.
async fn login() -> Result<(), ApiError> {
    let ctx = ErrorContext::new("auth-service")
        .with_trace("4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7")
        .with_meta("route", "POST /v1/sessions");

    Err(DistributedError::new(AuthError::TokenExpired, ctx).into())
}
```

A fully compilable version of this pattern lives in [`examples/auth_service.rs`](examples/auth_service.rs).

```bash
cargo run -p error --example auth_service
```

---

## ⚙️ Configuration & Runtime Environment

This crate is a **stateless library** — it has no runtime configuration, no environment variables, and no background threads. All operational knobs (log format, sampling, alerting thresholds) live in the consuming service's bootstrap.

| Variable | Required | Default | Description |
|---|---|---|---|
| *(none)* | — | — | No environment variables are consumed by this crate. |

### Compile-time

| Cargo feature | Default | Description |
|---|---|---|
| *(none declared)* | — | The crate has no optional feature flags. All functionality is always compiled. |

### Axum integration note

`axum` appears **only in `[dev-dependencies]`** (used by the example). Adding the `error` crate to a non-axum service incurs no axum dependency.

---

## 📈 Telemetry, Performance & Metrics

### Runtime prerequisites

- **Async runtime:** None required. The crate is synchronous; `DistributedError::log()` calls `tracing` macros which are non-blocking by design.
- **`tracing` subscriber:** Must be installed by the consuming service before `.log()` events are useful. Without a subscriber, events are silently dropped (standard `tracing` behavior).
- **No CPU architecture constraints.**

### Structured log event emitted by `DistributedError::log()`

Every call to `.log()` emits a single `tracing` event at the level dictated by `Severity::log_level()`:

| Field | Type | Source |
|---|---|---|
| `request_id` | `Uuid` | `ErrorContext::request_id` |
| `trace_id` | `Option<String>` | `ErrorContext::trace_id` |
| `span_id` | `Option<String>` | `ErrorContext::span_id` |
| `service` | `&'static str` | `ErrorContext::service_name` |
| `severity` | `String` | `AppError::severity()` |
| `error_code` | `&'static str` | `AppError::error_code()` |
| `category` | `&'static str` | `AppError::category()` |
| `retryable` | `bool` | `AppError::is_retryable()` |
| `error.message` | `String` | `Display` impl of the error |

### Recommended production alerts

| Alert | Condition | Severity |
|---|---|---|
| **Error surge** | Rate of `error_code != ""` log events spikes > baseline × 5 in 1 min | Page |
| **Critical/High errors** | Any event with `severity = "Critical"` or `severity = "High"` | Page immediately |
| **Non-retryable upstream failures** | `category = "DB"` or `category = "UPSTREAM"` with `retryable = false` > threshold | Page |
| **Retryable storm** | `retryable = true` rate > threshold (indicates upstream instability) | Warn |

> Correlate by `request_id` (client-visible) or by `trace_id` + `span_id` (internal, in logs) to reconstruct distributed call chains.

---

## 🛠️ Local Development & Contribution

### Prerequisites

No external services required — this is a pure Rust library.

```bash
# Verify Rust toolchain
rustup show
```

### Build & check

```bash
# Compile
cargo build -p error

# Type-check without linking (fast)
cargo check -p error

# Lint
cargo clippy -p error -- -D warnings

# Format
cargo fmt -p error
```

### Tests

```bash
# Unit tests (inline in source modules)
cargo test -p error

# Run the end-to-end example
cargo run -p error --example auth_service
```

### Adding a new method to `AppError`

1. Add the method to `traits.rs` with a **conservative default** — this is the only way to remain backward-compatible with all existing implementors.
2. Update `DistributedError::log()` in `context.rs` if the new field should appear in log events.
3. Update `ApiErrorResponse::from_error()` in `http.rs` if the field should appear in client responses.
4. Document the stability contract: is the new field part of the public API (breaking if removed)?

---

## 🚨 Troubleshooting & Runbook

### 1. `impl IntoResponse for DistributedError<MyError>` fails to compile

**Symptom:** Compiler error: *"only traits defined in the current crate can be implemented for types defined outside of the crate."*

**Root cause:** Orphan rule. `IntoResponse` is from `axum` and `DistributedError` is from `error` — both are foreign to your service crate. Parameterizing `DistributedError` with a local type is not sufficient to make the impl local because `DistributedError` is not `#[fundamental]`.

**Fix:** Wrap in a service-owned newtype:

```rust
pub struct ApiError(pub DistributedError<MyError>);
impl From<DistributedError<MyError>> for ApiError { ... }
impl IntoResponse for ApiError { ... }
```

See [`examples/auth_service.rs`](examples/auth_service.rs) for the full pattern.

---

### 2. `.log()` calls produce no output

**Symptom:** `DistributedError::log()` is called but nothing appears in stdout/stderr.

**Root cause:** No `tracing` subscriber is installed. `tracing` events are silently discarded when no subscriber is active.

**Fix:** Install a subscriber in the service's `main` or test setup:

```rust
tracing_subscriber::fmt::init();  // development / example use
```

For production, use a structured JSON subscriber wired to your log aggregator.

---

### 3. `trace_id` / `span_id` appear in client responses

**Symptom:** An API client receives `trace_id` or `span_id` in the JSON error body.

**Root cause:** The service is not using `into_api_response()` / `ApiErrorResponse::from_error()` — it is serializing `ErrorContext` or `DistributedError` directly.

**Fix:** Always build the client response through the provided helpers:

```rust
let body = into_api_response(&distributed_err);  // trace/span ids are stripped here
// or:
let body = my_error.to_api_response(&ctx);        // same guarantee via blanket impl
```

Never serialize `ErrorContext` or `DistributedError` directly into an HTTP response.
