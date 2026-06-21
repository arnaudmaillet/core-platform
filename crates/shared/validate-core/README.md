# validate-core — Zero-dependency validation abstraction

## 🎯 Overview & Service Role

`validate-core` is the workspace-wide **validation abstraction boundary**. It defines exactly two public items:

- **`FieldViolation`** — a single field-level constraint failure carrying a stable `VAL-xxxx` code, a dot-notation field path, and a human-readable message.
- **`Validate`** — a trait that any type can implement to express its own invariant checks, returning either `Ok(())` or `Err(Vec<FieldViolation>)` with a complete picture of every failure in one pass.

Its sole purpose is to allow `cqrs` (which adds `Validate` as a `Command` supertrait) and `validation` (which provides the full middleware and error types) to both point inward toward a shared abstraction without depending on each other. Zero external dependencies is a hard constraint — it must never grow a dependency.

## 📐 Architecture & Concepts

```
       validate-core          (zero deps — the abstraction)
        ▲            ▲
        │            │
      cqrs       validation
  (requires      (provides
  Validate as     ValidationLayer
  Command         + ValidationError
  supertrait)     + VAL-xxxx codes)
```

**Why a separate crate and not a module inside `validation`?**
If `Validate` lived in `validation`, then `cqrs` would depend on `validation`, pulling in middleware, HTTP status codes, and error-framework machinery. `cqrs` would own the bus protocol and the operational stack — a violation of the Single Responsibility Principle. `validate-core` is the Separated Interface pattern: both sides of the dependency graph converge on this thin abstraction.

**Aggregation, not short-circuit:**
`validate()` is required to collect **all** violations before returning. A client that submits a form with three invalid fields must receive three error codes in a single round-trip, not one per request.

## 🔌 Public Interfaces & API Contract

```rust
/// A single field-level constraint failure.
pub struct FieldViolation {
    pub field:   &'static str,   // dot-notation path, e.g. "user.email"
    pub code:    &'static str,   // stable VAL-xxxx code, e.g. "VAL-1001"
    pub message: String,         // human-readable explanation
}

impl FieldViolation {
    pub fn new(field: &'static str, code: &'static str, message: impl Into<String>) -> Self;
}

/// Self-validation contract for any type.
pub trait Validate {
    /// Default implementation returns Ok(()) — override when constraints exist.
    fn validate(&self) -> Result<(), Vec<FieldViolation>> { Ok(()) }
}
```

**Invariants:**
- `field` and `code` are `&'static str` — no heap allocation per violation on the hot validation path.
- A returned `Err(violations)` vec must be non-empty by convention (enforced with `debug_assert!` in `ValidationError::new` in the `validation` crate).
- `Validate` is a supertrait of `cqrs::Command`. Every command automatically satisfies it via the default no-op unless explicitly overridden.

## 📦 Integration & Usage

```toml
# Cargo.toml
[dependencies]
validate-core = { workspace = true }
```

**Implementing on a command:**

```rust
use validate_core::{FieldViolation, Validate};

pub struct CreateUserCommand {
    pub username: String,
    pub age:      u8,
}

impl Validate for CreateUserCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();

        if self.username.is_empty() {
            v.push(FieldViolation::new("username", "VAL-1001", "must not be empty"));
        }
        if self.username.len() > 32 {
            v.push(FieldViolation::new("username", "VAL-1002", "must be at most 32 characters"));
        }
        if self.age < 13 {
            v.push(FieldViolation::new("age", "VAL-1004", "must be at least 13"));
        }

        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}
```

**Command with no validation (uses the no-op default):**

```rust
use validate_core::Validate;

pub struct PingCommand;
impl Validate for PingCommand {}  // impl Command for PingCommand is sufficient
```

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| — | — | — | This crate has no runtime configuration. It is a pure compile-time abstraction. |

**Cargo features:** None. The zero-dependency guarantee is enforced by keeping the feature set empty.

## 📈 Telemetry, Performance & Metrics

- **No async runtime dependency.** `validate()` is a synchronous, infallible function call on the hot path.
- **Allocation cost:** one `Vec` allocation only when violations are found. The happy path (`Ok(())`) allocates nothing.
- **No metrics exposed.** Violation counts and field-level telemetry are the responsibility of the `ValidationLayer` in the `validation` crate.

## 🛠️ Local Development & Contribution

```bash
# Build
cargo build -p validate-core

# Lint
cargo clippy -p validate-core -- -D warnings

# Test (doc-tests only — no unit tests by design)
cargo test -p validate-core
```

**Hard constraints for contributors:**
1. `[dependencies]` in `Cargo.toml` must remain empty.
2. No `use` of `std::collections`, async types, or error-framework types.
3. `FieldViolation` fields (`field`, `code`) must remain `&'static str` — never `String`.

## 🚨 Troubleshooting & Runbook

**`validate_core::Validate` is not implemented for `MyCommand` and `Command` requires it.**
Every type that implements `cqrs::Command` must also implement `Validate`. Add `impl Validate for MyCommand {}` to use the no-op default, or provide a real implementation if the command carries user-supplied data.

**`violations` vec is empty but `ValidationError::new` panicked in debug mode.**
`ValidationError::new` asserts the vec is non-empty. Your `validate()` implementation must only return `Err(v)` when `v` contains at least one `FieldViolation`. Guard with `if v.is_empty() { Ok(()) } else { Err(v) }`.
