# `validate-core` — Zero-dependency validation abstraction shared by `cqrs` and `validation`

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — the validation abstraction boundary (Separated Interface) |
> | **Package** | `validate-core` (dir: `crates/foundation/validate-core`) |
> | **Consumed by** | `cqrs` (as a `Command` supertrait), `validation` (middleware + error types) |
> | **Depends on** | **nothing** — zero dependencies is a hard, enforced constraint |
> | **Stability** | stable contract |
> | **Feature flags** | none (empty feature set enforces zero-dep) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`validate-core` is the workspace-wide **validation abstraction boundary**. It defines exactly two
public items: `FieldViolation` (a single field-level failure carrying a stable `VAL-xxxx` code, a
dot-notation field path, and a message) and `Validate` (a trait any type implements to express its
own invariant checks, returning `Ok(())` or `Err(Vec<FieldViolation>)`).

**Architectural boundary** — it exists so `cqrs` and `validation` can both point inward at a shared
abstraction **without depending on each other**. It must never grow a dependency, never pull in
middleware, HTTP types, or error-framework machinery.

---

## 📐 Architecture & key decisions

```
       validate-core          (zero deps — the abstraction)
        ▲            ▲
        │            │
      cqrs       validation
  (Validate as    (ValidationLayer + ValidationError + VAL-xxxx codes)
   Command supertrait)
```

- **Separated Interface, not a module in `validation`** — if `Validate` lived in `validation`, `cqrs`
  would depend on `validation` and inherit its middleware/HTTP/error stack, violating SRP. A thin
  third crate lets both sides of the graph converge without coupling.
- **Aggregation, not short-circuit** — `validate()` must collect **all** violations before returning.
  A form with three bad fields yields three codes in one round-trip, not one error per submit.
- **`&'static str` fields** — `field` and `code` are static, so a violation allocates nothing on the
  validation hot path; only the `Vec` allocates, and only when there *is* a violation.

---

## 🔌 Public API & contract

```rust
pub struct FieldViolation {
    pub field:   &'static str,   // dot-notation path, e.g. "user.email"
    pub code:    &'static str,   // stable VAL-xxxx code, e.g. "VAL-1001"
    pub message: String,
}
impl FieldViolation { pub fn new(field: &'static str, code: &'static str, message: impl Into<String>) -> Self; }

pub trait Validate {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> { Ok(()) }   // default no-op — override when constrained
}
```

> **Contract notes:** `Validate` is a supertrait of `cqrs::Command`, so every command satisfies it via
> the default unless overridden. A returned `Err(violations)` must be **non-empty** by convention
> (`ValidationError::new` in the `validation` crate `debug_assert!`s this). `field`/`code` must stay
> `&'static str` — never `String`.

---

## 📦 Integration

```toml
[dependencies]
validate-core = { workspace = true }
```

```rust
use validate_core::{FieldViolation, Validate};

impl Validate for CreateUserCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.username.is_empty()   { v.push(FieldViolation::new("username", "VAL-1001", "must not be empty")); }
        if self.username.len() > 32   { v.push(FieldViolation::new("username", "VAL-1002", "must be at most 32 characters")); }
        if self.age < 13              { v.push(FieldViolation::new("age", "VAL-1004", "must be at least 13")); }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

// A command with no constraints uses the no-op default:
impl Validate for PingCommand {}
```

---

## ⚙️ Configuration & feature flags

None. No runtime config, no env vars, no cargo features — the empty feature set is what *enforces* the
zero-dependency guarantee.

---

## 🧪 Testing

```bash
cargo test   -p validate-core          # doc-tests only (no unit tests by design)
cargo clippy -p validate-core --all-targets
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `Validate is not implemented for MyCommand` (and `Command` requires it).**
Every `cqrs::Command` must also implement `Validate`. Add `impl Validate for MyCommand {}` for the
no-op default, or a real impl if the command carries user-supplied data.

**2. `ValidationError::new` panicked in debug though my `violations` vec was empty.**
It `debug_assert!`s the vec is non-empty. `validate()` must only return `Err(v)` when `v` has ≥ 1
violation — guard with `if v.is_empty() { Ok(()) } else { Err(v) }`.

**3. A PR added a dependency / a `std::collections` import and CI/review pushed back.**
By design: `[dependencies]` must remain empty and no `use` of `std::collections`, async, or
error-framework types is allowed. The zero-dep constraint is the crate's entire reason to exist.
