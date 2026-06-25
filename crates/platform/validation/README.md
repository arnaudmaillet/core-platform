# `validation` — Tower-inspired input-validation middleware for the CQRS command bus

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `platform` — the operational half of validation (the [`validate-core`](../../foundation/validate-core) mechanism made into middleware) |
> | **Package** | `validation` (dir: `crates/platform/validation`) |
> | **Consumed by** | service composition roots (placed outermost on the command pipeline) |
> | **Depends on** | `validate-core`, `cqrs`, `error` |
> | **Stability** | stable contract |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`validation` is the operational half of the platform's input-validation system. It turns the abstract
`Validate` contract from [`validate-core`](../../foundation/validate-core) into: `ValidationError` (a
concrete `AppError` → HTTP 422, `Severity::Low`, carrying a `Vec<FieldViolation>` + a
`to_details_map()`), `ValidationLayer` (a zero-size `CommandLayer` that calls `validate()` before
dispatch and short-circuits on failure), and the stable `VAL-xxxx` constant catalogue.

**Architectural boundary** — it owns the *middleware + error type*; the `Validate` trait and
`FieldViolation` live in `validate-core` (so `cqrs` can depend on the abstraction without this crate's
middleware). Failures are rejected at the **earliest** point — before any span, idempotency record, or
DB transaction — eliminating wasted work at scale.

---

## 📐 Architecture & key decisions

```
Inbound command
   ▼ ValidationCommandBus (OUTERMOST)
   │   envelope.payload.validate()
   │     ├─ Ok(())            ─► inner pipeline
   │     └─ Err(violations)   ─► CqrsError::Handler(ValidationError)   (handler never called)
   ▼ IdempotencyCommandBus ─► TracingCommandBus ─► LoggingCommandBus ─► InMemoryCommandBus ─► handler
```

- **Outermost on purpose** — rejecting before idempotency/tracing/DB means an invalid command consumes
  no async resources (rejection happens before the first `.await`).
- **Zero-cost for valid commands** — `ValidationLayer` is zero-size; a single inlined `validate()` call
  that the compiler eliminates entirely for commands using the no-op default.
- **Aggregated field errors** — the `Vec<FieldViolation>` + `to_details_map()` returns every failing
  field in one round-trip, serializing straight into `ApiErrorResponse.details`.

---

## 🔌 Public API & contract

```rust
pub struct ValidationError { /* violations: Vec<FieldViolation> */ }
impl ValidationError {
    pub fn new(violations: Vec<FieldViolation>) -> Self;     // debug-panics if empty
    pub fn violations(&self) -> &[FieldViolation];
    pub fn to_details_map(&self) -> HashMap<String, String>; // field → "VAL-xxxx: message"
}
// impl AppError: error_code "VAL-0001", http_status 422, severity Low, retryable false, category "VALIDATION"

#[derive(Default, Clone, Copy)] pub struct ValidationLayer;  // zero-size CommandLayer
impl<S> CommandLayer<S> for ValidationLayer { type Service = ValidationCommandBus<S>; /* … */ }

// VAL-xxxx constants:
pub const VAL_1001_REQUIRED: &str = "VAL-1001"; // …1002 LENGTH, 1003 PATTERN, 1004 RANGE, 1005 EMAIL,
pub const VAL_9000_CUSTOM:   &str = "VAL-9000"; //   1006 URL, 1007 ENUM, 1008 SIZE, 1009 UNIQUE, 9000 catch-all
```

> **Contract notes:** `ValidationLayer` must be the **outermost** layer. `CqrsError::Handler` wraps a
> type-erased `BoxedDynAppError` — you **cannot** downcast it back to `ValidationError`; inspect via
> `error_code()` (`"VAL-0001"`) and `Display`.

---

## 📦 Integration

```toml
[dependencies]
validation    = { workspace = true }
validate-core = { workspace = true }   # for Validate + FieldViolation on your command types
```

```rust
use validation::ValidationLayer;
// ValidationLayer FIRST = outermost (MiddlewarePipeline applies layers inside-out).
let bus = MiddlewarePipeline::new(inner)
    .layer(ValidationLayer)
    .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
    .layer(TracingLayer)
    .layer(LoggingLayer)
    .build();

let result = bus.dispatch(Envelope::new(correlation_id, cmd)).await; // invalid ⇒ Err(CqrsError::Handler(_))
```

---

## ⚙️ Configuration & feature flags

None — no environment variables and no cargo features. Pipeline placement is a composition-root
decision, not config.

---

## 🔭 Observability

One `tracing` event: `command validation failed — dispatch short-circuited` (`DEBUG`, fields
`command.type`, `violation.count`). DEBUG is intentional — validation failures are expected client
behaviour, not incidents. Filter dashboards on `category = "VALIDATION"` / `Severity::Low`.

Suggested alert: `error_code = "VAL-0001"` spiking above baseline ⇒ possible client breaking change or
a bad deploy pushing malformed data. Indicative overhead: ~0 ns valid, ~50–200 ns invalid.

---

## 🧪 Testing

```bash
cargo test   -p validation
cargo clippy -p validate-core -p validation --all-targets
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `ValidationLayer` rejects commands that should be valid.**
Your `Validate` impl returns `Err(_)` unexpectedly. Call `cmd.validate()` directly in a unit test and
inspect the `Vec<FieldViolation>`. Common cause: a length check using `.len()` (bytes) vs
`.chars().count()` (code points) on non-ASCII input.

**2. `CqrsError::Handler` returned but downcast to `ValidationError` fails.**
`Handler` wraps a type-erased `BoxedDynAppError` — you can't downcast after wrapping. Read
`error_code()` (`"VAL-0001"`) and `Display` for field details; if you need structured access, validate
explicitly before dispatch and map the result yourself.

**3. Another layer intercepts before `ValidationLayer`.**
`MiddlewarePipeline::layer()` applies inside-out — the **first** `.layer()` call is outermost. Call
`.layer(ValidationLayer)` first.
