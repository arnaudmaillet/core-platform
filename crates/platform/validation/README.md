# validation — Tower-inspired input validation middleware for the CQRS command bus

## 🎯 Overview & Service Role

`validation` is the **operational half** of the platform's input validation system. It takes the abstract `Validate` contract defined in `validate-core` and turns it into:

- **`ValidationError`** — a concrete, [`AppError`](../error)-implementing error type that maps to HTTP 422 Unprocessable Entity with `Severity::Low`. Carries a full `Vec<FieldViolation>` and exposes a `to_details_map()` serialisable directly into `ApiErrorResponse.details`.
- **`ValidationLayer`** — a zero-size Tower-inspired `CommandLayer` that intercepts every command before dispatch, calls `validate()` on the payload, and short-circuits with a `CqrsError::Handler(ValidationError)` if any violations are found. Zero overhead for valid commands — no heap allocation, no type-map lookup, full monomorphisation.
- **VAL-xxxx constants** — a stable, machine-readable catalogue of constraint codes for use across the entire platform.

**Business impact:** Validation failures are rejected at the earliest possible point in the pipeline — before any tracing span, idempotency record, or database transaction is opened. This eliminates wasted work at scale and returns precise, field-level error maps to clients in a single round-trip.

## 📐 Architecture & Concepts

```
  Inbound command
        │
        ▼
┌─────────────────────────────────────────────┐
│  ValidationCommandBus  (outermost layer)    │
│                                             │
│  envelope.payload.validate()                │
│     ├─ Ok(())  ──────────────────────────── │──► inner pipeline
│     └─ Err(violations)                      │
│           │                                 │
│           ▼                                 │
│  CqrsError::Handler(ValidationError)  ◄──── │── short-circuit (handler never called)
└─────────────────────────────────────────────┘
        │
        ▼
  IdempotencyCommandBus
        │
  TracingCommandBus
        │
  LoggingCommandBus
        │
  InMemoryCommandBus  ──► registered handler
```

### Resilience Guarantees & High-Load Behaviour

| Concern | Behaviour |
|---|---|
| **CPU overhead (valid commands)** | Single inlined `validate()` call. Compiler eliminates it for commands with the no-op default. |
| **CPU overhead (invalid commands)** | One `Vec` allocation for violations + one `CqrsError` construction. No handler invocation, no span creation, no idempotency write. |
| **Memory** | `ValidationLayer` is zero-size. `ValidationCommandBus<S>` holds only `S`. No shared state, no registry, no `Arc`. |
| **Concurrency** | Fully `Send + Sync`. No internal mutability. |
| **Backpressure** | Validation runs synchronously before any async `.await` point. Rejection happens before the Tokio task yields, so no async resources are consumed for invalid requests. |

## 🔌 Public Interfaces & API Contract

### `ValidationError`

```rust
pub struct ValidationError { /* violations: Vec<FieldViolation> */ }

impl ValidationError {
    /// Wraps a non-empty list of violations. Panics in debug if empty.
    pub fn new(violations: Vec<FieldViolation>) -> Self;

    /// Read access to the collected violations.
    pub fn violations(&self) -> &[FieldViolation];

    /// Serialisable map of field → "VAL-xxxx: message" for ApiErrorResponse.details.
    pub fn to_details_map(&self) -> HashMap<String, String>;
}

impl AppError for ValidationError {
    fn error_code()           -> &'static str  { "VAL-0001" }
    fn http_status()          -> StatusCode     { 422 Unprocessable Entity }
    fn severity()             -> Severity       { Severity::Low }
    fn is_retryable()         -> bool           { false }
    fn category()             -> &'static str  { "VALIDATION" }
    fn user_facing_message()  -> &'static str  { "One or more fields failed validation…" }
}
```

### `ValidationLayer`

```rust
/// Zero-size CommandLayer marker.
#[derive(Debug, Clone, Copy, Default)]
pub struct ValidationLayer;

impl<S> CommandLayer<S> for ValidationLayer {
    type Service = ValidationCommandBus<S>;
    fn layer(&self, inner: S) -> ValidationCommandBus<S>;
}

impl<S: CommandBus> CommandBus for ValidationCommandBus<S> {
    fn dispatch<C: Command>(&self, envelope: Envelope<C>)
        -> impl Future<Output = Result<(), CqrsError>> + Send + '_;
}
```

### VAL-xxxx code constants

```rust
pub const VAL_1001_REQUIRED: &str = "VAL-1001";  // required / absent / empty
pub const VAL_1002_LENGTH:   &str = "VAL-1002";  // length out of bounds
pub const VAL_1003_PATTERN:  &str = "VAL-1003";  // regex / format mismatch
pub const VAL_1004_RANGE:    &str = "VAL-1004";  // numeric range violation
pub const VAL_1005_EMAIL:    &str = "VAL-1005";  // malformed e-mail
pub const VAL_1006_URL:      &str = "VAL-1006";  // malformed URL
pub const VAL_1007_ENUM:     &str = "VAL-1007";  // unknown enum variant
pub const VAL_1008_SIZE:     &str = "VAL-1008";  // collection size out of bounds
pub const VAL_1009_UNIQUE:   &str = "VAL-1009";  // uniqueness violation
pub const VAL_9000_CUSTOM:   &str = "VAL-9000";  // catch-all
```

## 📦 Integration & Usage

```toml
# Cargo.toml
[dependencies]
validation    = { workspace = true }
validate-core = { workspace = true }  # for Validate + FieldViolation on your command types
```

### Standard Bootstrap Pattern

```rust
use cqrs::{CommandBus, CommandBusBuilder, Envelope, MiddlewarePipeline,
           IdempotencyLayer, InMemoryIdempotencyStore, LoggingLayer, TracingLayer};
use validation::ValidationLayer;
use uuid::Uuid;

// 1. Build the inner handler registry.
let inner = CommandBusBuilder::new()
    .register::<CreateUserCommand, _>(CreateUserHandler::new(repo))?
    .build();

// 2. Compose the middleware pipeline.
//    ValidationLayer is outermost — rejects before any other work runs.
let bus = MiddlewarePipeline::new(inner)
    .layer(ValidationLayer)
    .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
    .layer(TracingLayer)
    .layer(LoggingLayer)
    .build();

// 3. Dispatch. Invalid commands return Err(CqrsError::Handler(_)) immediately.
let result = bus.dispatch(Envelope::new(correlation_id, cmd)).await;
```

### Mapping `ValidationError` to an API response

```rust
use error::{ApiErrorResponse, AppError, ErrorContext};
use cqrs::CqrsError;

fn map_to_api_response(err: CqrsError, ctx: &ErrorContext) -> ApiErrorResponse {
    let mut response = ApiErrorResponse::from_error(&err, ctx);
    // Enrich the details map with field-level violations when available.
    if let CqrsError::Handler(ref boxed) = err {
        // The details map is already populated by ValidationError::to_details_map()
        // via the Display impl — use it directly if your handler stores it.
    }
    response
}
```

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| — | — | — | This crate has no environment variables. All configuration is compile-time via Cargo features. |

**Cargo features:** None defined. The crate activates its full surface by default.

**Cargo feature flags (external):**
`ValidationLayer` has zero runtime configuration. Pipeline placement is determined at the composition root in your application binary, not via environment variables.

## 📈 Telemetry, Performance & Metrics

**Runtime requirements:** Tokio async runtime (multi-thread or current-thread). The `dispatch` future is `Send` and safe to `.await` in any Tokio task.

**OTel / tracing events emitted by `ValidationLayer`:**

| Event | Level | Fields |
|---|---|---|
| `command validation failed — dispatch short-circuited` | `DEBUG` | `command.type`, `violation.count` |

The DEBUG level is intentional: validation failures are expected client behaviour, not operational incidents. Use `Severity::Low` and `category = "VALIDATION"` in your alerting rules to filter these out of error-rate dashboards.

**Recommended production alert:** Alert when `error_code = "VAL-0001"` spikes above your baseline — a sudden increase may indicate a client-library breaking change or a malformed deployment pushing bad data.

**Performance benchmarks (indicative):**
- Valid command overhead: `~0 ns` (no-op `validate()` is inlined to nothing by the compiler for default impls).
- Invalid command overhead: `~50–200 ns` (one `Vec` alloc for violations + `CqrsError` construction).

## 🛠️ Local Development & Contribution

```bash
# Build both crates
cargo build -p validate-core -p validation

# Lint (warnings are errors in CI)
cargo clippy -p validate-core -p validation -- -D warnings

# Run integration tests
cargo test -p validation

# Format
cargo fmt -p validate-core -p validation
```

**Test coverage targets:**

| Scenario | Test |
|---|---|
| Valid command reaches inner bus | `valid_command_passes_through_to_inner_bus` |
| Invalid command does NOT reach inner bus | `invalid_command_short_circuits_before_inner_bus` |
| Error is `CqrsError::Handler` with HTTP 422 | `invalid_command_returns_cqrs_handler_error_with_val_code` |
| Single violation field and code preserved | `single_violation_field_and_code_are_preserved` |
| Multiple violations fully aggregated | `multiple_violations_are_all_reported` |
| `to_details_map()` shape | `validation_error_details_map_contains_all_fields` |
| Composes with `LoggingLayer` + `TracingLayer` | `validation_layer_composes_with_logging_and_tracing_layers` |

## 🚨 Troubleshooting & Runbook

**`ValidationLayer` rejects commands that should be valid.**
Your `Validate` impl is returning `Err(_)` unexpectedly. Add a unit test that calls `cmd.validate()` directly and inspect the returned `Vec<FieldViolation>`. Common cause: a length check using `.chars().count()` vs. `.len()` mismatch on non-ASCII input.

**`CqrsError::Handler` returned but downcast to `ValidationError` fails.**
The `CqrsError::Handler` variant wraps a `BoxedDynAppError` (type-erased). You cannot downcast it to `ValidationError` after wrapping. Inspect via `error_code()` (`"VAL-0001"`) and the `Display` output to get field details. If you need structured access to violations before type erasure, validate explicitly before dispatching and map the result yourself.

**`ValidationLayer` must be the outermost layer but another layer is intercepting first.**
`MiddlewarePipeline::layer()` applies layers inside-out: the first `.layer()` call is the outermost decorator. Always call `.layer(ValidationLayer)` **first** in the chain. See the bootstrap pattern above.
