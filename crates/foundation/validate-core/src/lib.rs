//! Zero-dependency validation abstraction for the core-platform workspace.
//!
//! This crate is intentionally minimal: it holds the [`Validate`] trait and
//! the [`FieldViolation`] primitive that carries a single field-level failure.
//! No error framework, no HTTP mapping, no async runtime — nothing that would
//! force a business-logic crate to pull in operational machinery just to
//! describe what it validates.
//!
//! ## Dependency inversion
//!
//! Both `cqrs` (which requires [`Validate`] as a [`Command`] supertrait) and
//! `validation` (which provides the middleware and full error type) depend on
//! this crate. Neither depends on the other, so the dependency arrows point
//! inward toward this abstraction.
//!
//! ## Implementing `Validate`
//!
//! ```rust
//! use validate_core::{FieldViolation, Validate};
//!
//! struct CreateUserCommand {
//!     username: String,
//!     age: u8,
//! }
//!
//! impl Validate for CreateUserCommand {
//!     fn validate(&self) -> Result<(), Vec<FieldViolation>> {
//!         let mut violations = Vec::new();
//!
//!         if self.username.is_empty() {
//!             violations.push(FieldViolation::new("username", "VAL-1001", "must not be empty"));
//!         }
//!         if self.age < 13 {
//!             violations.push(FieldViolation::new("age", "VAL-1004", "must be at least 13"));
//!         }
//!
//!         if violations.is_empty() { Ok(()) } else { Err(violations) }
//!     }
//! }
//! ```
//!
//! Commands that need no validation provide the default no-op:
//!
//! ```rust
//! use validate_core::Validate;
//!
//! struct PingCommand;
//! impl Validate for PingCommand {}
//! ```

/// A single field-level constraint failure.
///
/// Carries the minimum information required to tell a client exactly what
/// was wrong: which field, which rule, and a human-readable explanation.
/// The `code` is stable and machine-readable; `message` is for developers
/// and end-users. The full [`ValidationError`](../validation/struct.ValidationError.html)
/// type in the `validation` crate wraps a `Vec<FieldViolation>` and implements
/// [`AppError`](../error/trait.AppError.html).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldViolation {
    /// Dot-notation path to the offending field, e.g. `"user.email"`.
    pub field: &'static str,

    /// Stable, machine-readable violation code in the `VAL-xxxx` namespace,
    /// e.g. `"VAL-1001"`. Clients and dashboards key off this value;
    /// treat changes as breaking.
    pub code: &'static str,

    /// Human-readable explanation of why the constraint was violated.
    /// May be shown to end-users after sanitisation by the API layer.
    pub message: String,
}

impl FieldViolation {
    /// Constructs a new [`FieldViolation`].
    ///
    /// `field` and `code` are `&'static str` so callers use constants rather
    /// than heap-allocating identifiers on every validation pass.
    pub fn new(field: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            field,
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FieldViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} — {}", self.field, self.code, self.message)
    }
}

/// Contract for types that can validate their own invariants.
///
/// Implementors inspect `self` and return either `Ok(())` when all constraints
/// are satisfied, or `Err(violations)` carrying every failed field — never
/// short-circuit on the first failure so the caller receives the complete
/// picture in one pass.
///
/// ## Default implementation
///
/// The provided default returns `Ok(())`, making it a no-op for types that
/// carry no user-supplied data and therefore need no validation (e.g. internal
/// system commands). Override only when meaningful constraints exist.
///
/// ## Supertrait on `Command`
///
/// `cqrs::Command` lists `Validate` as a supertrait so the [`ValidationLayer`]
/// middleware can call `validate()` generically on any `C: Command` with zero
/// dynamic dispatch overhead.
pub trait Validate {
    /// Validates `self` and collects all field violations.
    ///
    /// Returns `Ok(())` if every constraint is satisfied, or
    /// `Err(violations)` with a non-empty `Vec` otherwise.
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        Ok(())
    }
}
