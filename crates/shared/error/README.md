# `error` — Interface Contract

## 🎯 Responsibility

Provide the shared infrastructure primitives (traits, distributed context, serialization format) that let every microservice define its own error enums in a standardized, observable, and decoupled way — with no business logic and no application-layer dependencies.

---

## 🔌 Public Interfaces (Traits & API)

### `AppError` — required contract on every service error enum

```rust
pub trait AppError: std::error::Error + Send + Sync + 'static {
    // Required
    fn error_code(&self) -> &'static str;       // e.g. "AUTH_TOKEN_EXPIRED"
    fn http_status(&self) -> StatusCode;

    // Conservative defaults (override where needed)
    fn severity(&self) -> Severity { Severity::Medium }
    fn is_retryable(&self) -> bool { false }
    fn category(&self) -> &'static str { "UNKNOWN" }
    fn user_facing_message(&self) -> &'static str { "An error occurred." }
}
```

### `IntoApiResponse` — blanket impl on every `AppError + Sized`

```rust
pub trait IntoApiResponse: AppError + Sized {
    fn to_api_response(&self, ctx: &ErrorContext) -> ApiErrorResponse;
    // free implementation — do not override
}
```

### `ErrorContext` — distributed context, built once per request

```rust
ErrorContext::new("my-service")                    // generates request_id + timestamp
    .with_trace("trace_id", "span_id")             // OpenTelemetry / Jaeger
    .with_meta("user_id", "u_123")                 // arbitrary fields → JSON details
```

### `DistributedError<E: AppError>` — type-safe error envelope

```rust
DistributedError::new(my_error, ctx)   // wraps the error with its context
distributed_err.log()                  // structured tracing event (includes trace/span ids)
                                       // log level = severity.log_level()
```

### `into_api_response` — framework-agnostic client JSON rendering

```rust
pub fn into_api_response<E: AppError>(err: &DistributedError<E>) -> ApiErrorResponse
```

### `Severity` — shared urgency vocabulary

| Variant    | `log_level()` | `should_page()` |
|------------|---------------|-----------------|
| `Critical` | `ERROR`       | `true`          |
| `High`     | `ERROR`       | `true`          |
| `Medium`   | `WARN`        | `false`         |
| `Low`      | `INFO`        | `false`         |
| `Info`     | `DEBUG`       | `false`         |

### `ApiErrorResponse` — the only JSON payload exposed to clients

```
error_code | message | request_id | service | severity | retryable | category | timestamp | details
```
> `trace_id` and `span_id` are **never** included — they stay in logs only.

---

## 📦 Entry Points & Consumption

**This crate consumes nothing** (no Kafka, no gRPC, no inbound HTTP). It is a pure library: microservices depend on it, not the other way around.

**Runtime injection required:** none. The crate is stateless.

**To add it to a service:**

```toml
# service Cargo.toml
error = { path = "crates/shared/error" }
```

**axum integration pattern (newtype required — orphan rule):**

```rust
// 1. Define the error enum and implement AppError
#[derive(Debug, thiserror::Error)]
pub enum MyError { ... }
impl AppError for MyError { ... }

// 2. axum newtype (required — DistributedError is a foreign type)
pub struct ApiError(pub DistributedError<MyError>);
impl From<DistributedError<MyError>> for ApiError { ... }

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        err.log();                              // full internal log (trace/span included)
        let status = err.error.http_status();
        let body = into_api_response(&err);     // safe client-facing payload
        (status, Json(body)).into_response()
    }
}
```

---

## 📝 Key Files

| File | Contents |
|------|----------|
| `src/traits.rs` | `AppError` (contract to implement) + `IntoApiResponse` (blanket impl) |
| `src/context.rs` | `ErrorContext` (builder) + `DistributedError<E>` (envelope + `.log()`) |
| `src/http.rs` | `ApiErrorResponse` (client JSON shape) + `into_api_response()` + commented axum pattern |
| `examples/auth_service.rs` | Full compilable end-to-end: auth-service with axum integration |

`src/severity.rs` does not need to be read to consume the crate — the variants are self-explanatory.
