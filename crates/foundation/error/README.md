# `error` — The platform's distributed-error contract: one trait, one wire shape, zero leakage

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — workspace-wide error contract, no domain logic |
> | **Package** | `error` (dir: `crates/foundation/error`) |
> | **Consumed by** | `cqrs`, `transport`, `resilience`, and every service crate |
> | **Depends on** | `thiserror`, `tracing`, `http`, `uuid`, `chrono` (axum only in dev-deps) |
> | **Stability** | stable contract (`error_code` is public API) |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`error` is the workspace-wide foundation for structured, observable, distributed error handling. It
provides the contract, vocabulary, and serialization primitives that let every microservice define
its **own** error enum independently while guaranteeing a uniform, client-safe, telemetry-rich output
at the platform boundary.

**Architectural boundary** — it defines **no business logic and no domain**. It has no network I/O and
no state, so it can never be the cause of a cascading failure. All operational knobs (log format,
sampling, alerting) live in the consuming service's bootstrap.

**Core objectives:** one trait / one JSON shape / one severity vocabulary regardless of which service
raised the error; zero information leakage (`trace_id`/`span_id` stay in logs, never reach clients);
type preservation end-to-end (`DistributedError<E>` keeps the concrete type — no `Box<dyn Error>`);
progressive disclosure (only two methods are mandatory on `AppError`).

---

## 📐 Architecture & key decisions

```
Service error enum (e.g. AuthError) ──implements──► AppError  (+ blanket IntoApiResponse)
        │ wrapped by
        ▼
DistributedError<E> { error: E (typed), context: ErrorContext }
        │ .log() ──► tracing event (trace/span ids INSIDE)
        │ into_api_response()
        ▼
ApiErrorResponse  ──JSON──►  API client   (NO trace_id / span_id)
```

The four pillars: **Contract** (`AppError` + `IntoApiResponse`), **Vocabulary** (`Severity`),
**Context** (`ErrorContext` + `DistributedError<E>`), **Wire format** (`ApiErrorResponse` +
`into_api_response`).

- **Typed envelope, no erasure** — `DistributedError<E>` keeps the concrete error type, so consumers
  keep full pattern-matching. The alternative (`Box<dyn Error>`) was rejected: it destroys the type
  the service needs to branch on.
- **Two-tier disclosure** — only `error_code()` and `http_status()` are required on `AppError`;
  everything else has a conservative production-safe default, so adding a method never breaks
  implementors.
- **No heap on the hot path** — `AppError` methods return `&'static str`; the envelope is
  stack-allocated until the service returns it. On latency-sensitive paths services may
  `Box<DistributedError<E>>` to keep the `Result` small (`#[allow(clippy::result_large_err)]`).
- **Leakage is structurally impossible** — `trace_id`/`span_id` live on `ErrorContext` (logged) but
  are *absent* from `ApiErrorResponse`. As long as you build the response through the helpers, they
  cannot reach a client.

---

## 🔌 Public API & contract

```rust
pub trait AppError: std::error::Error + Send + Sync + 'static {
    fn error_code(&self) -> &'static str;        // "AUTH_TOKEN_EXPIRED" — rename = breaking change
    fn http_status(&self) -> StatusCode;
    // optional, production-safe defaults:
    fn severity(&self) -> Severity { Severity::Medium }
    fn is_retryable(&self) -> bool { false }
    fn category(&self) -> &'static str { "UNKNOWN" }
    fn user_facing_message(&self) -> &'static str { "An error occurred." }
}

pub trait IntoApiResponse: AppError + Sized { fn to_api_response(&self, ctx: &ErrorContext) -> ApiErrorResponse; }
impl<E: AppError> IntoApiResponse for E {}        // blanket — do NOT override

pub struct ErrorContext { /* request_id, trace_id, span_id, service_name, timestamp, metadata */ }
pub struct DistributedError<E: AppError> { pub error: E, pub context: ErrorContext }
impl<E: AppError> DistributedError<E> { pub fn new(error: E, context: ErrorContext) -> Self; pub fn log(&self); }

pub struct ApiErrorResponse { /* error_code, message, request_id, service, severity, retryable, category, timestamp, details */ }
pub fn into_api_response<E: AppError>(err: &DistributedError<E>) -> ApiErrorResponse;  // trace/span stripped
```

`Severity` is the unified urgency vocabulary (`Critical`/`High` page; `Medium` default; `Low`/`Info`
don't). It implements `Ord` as **`Critical < High < Medium < Low < Info`** ("higher urgency = lower
value").

> **Stability contract:** `error_code` is part of the public API — clients and dashboards key off it.
> Treat any rename as a breaking change requiring a versioned migration. The `IntoApiResponse` blanket
> impl must not be overridden.

---

## 📦 Integration

```toml
[dependencies]
error = { workspace = true }
thiserror = { workspace = true }   # recommended for ergonomic error definitions
```

```rust
// 1. domain enum  2. impl AppError  3. (axum) newtype for the orphan rule  4. use `?`
pub struct ApiError(pub DistributedError<AuthError>);
impl From<DistributedError<AuthError>> for ApiError { fn from(e: DistributedError<AuthError>) -> Self { ApiError(e) } }
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        err.log();                          // structured log w/ trace+span ids
        let status = err.error.http_status();
        (status, Json(into_api_response(&err))).into_response()   // safe client JSON
    }
}
```

A fully compilable version lives in [`examples/auth_service.rs`](examples/auth_service.rs):
`cargo run -p error --example auth_service`.

---

## ⚙️ Configuration & feature flags

Stateless library — no environment variables, no background threads, no cargo features. `axum` is a
**dev-dependency only** (used by the example); adding `error` to a non-axum service pulls in no axum.

---

## 🔭 Observability

`DistributedError::log()` emits one `tracing` event at `severity().log_level()` with fields:
`request_id`, `trace_id`, `span_id`, `service`, `severity`, `error_code`, `category`, `retryable`,
`error.message`. Correlate by `request_id` (client-visible) or `trace_id` + `span_id` (logs only).

Suggested alerts: any `severity = Critical|High` ⇒ page; `error_code` event-rate spike > 5× baseline
⇒ page; sustained `retryable = true` rate ⇒ warn (upstream instability).

---

## 🗂️ Module layout

```
src/
├── traits.rs    AppError + IntoApiResponse (blanket)
├── context.rs   ErrorContext + DistributedError<E> + .log()
└── http.rs      ApiErrorResponse + into_api_response (client wire format)
```

---

## 🧪 Testing

```bash
cargo test   -p error                  # unit tests inline in source
cargo clippy -p error --all-targets
cargo run    -p error --example auth_service
```

No external services required — pure Rust library.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `impl IntoResponse for DistributedError<MyError>` fails to compile.**
Orphan rule — both `IntoResponse` (axum) and `DistributedError` (this crate) are foreign to your
service. Parameterizing with a local type doesn't help (`DistributedError` isn't `#[fundamental]`).
Wrap in a service-owned newtype `ApiError(pub DistributedError<MyError>)` (see the example).

**2. `.log()` produces no output.**
No `tracing` subscriber is installed; events are silently discarded. Install one in `main`/test setup
(`tracing_subscriber::fmt::init()` for dev; a JSON subscriber in prod).

**3. `trace_id` / `span_id` leaked into a client response.**
The service serialized `ErrorContext` or `DistributedError` directly. **Never** do that — always build
the client body through `into_api_response(&err)` (or `err.to_api_response(&ctx)`), which strips them.

**4. Adding a method to `AppError` broke implementors.**
New methods MUST carry a conservative default (`traits.rs`) — that's the only backward-compatible way.
Then update `DistributedError::log()` / `ApiErrorResponse` if the field should surface.
