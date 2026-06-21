//! Shared test infrastructure for the `validation` integration test suite.
//!
//! Provides:
//! - [`InlineCommandBus`] — a minimal `CommandBus` that records whether its
//!   inner dispatch was reached, without requiring a handler registry.
//! - [`AlwaysValidCommand`] — a `Command` whose `validate()` always returns `Ok(())`.
//! - [`AlwaysInvalidCommand`] — a `Command` whose `validate()` always returns
//!   a fixed set of `FieldViolation`s.
//! - [`MultiViolationCommand`] — a `Command` that returns multiple violations
//!   across different fields, for testing aggregated error shapes.

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cqrs::{Command, CommandBus, CqrsError, Envelope};
use validate_core::{FieldViolation, Validate};
use validation::{VAL_1001_REQUIRED, VAL_1002_LENGTH, VAL_1004_RANGE};

// ── InlineCommandBus ──────────────────────────────────────────────────────────

/// Stub [`CommandBus`] that immediately returns `Ok(())` and flips a flag so
/// tests can assert that the inner bus was (or was not) reached.
#[derive(Clone, Default)]
pub struct InlineCommandBus {
    pub reached: Arc<AtomicBool>,
}

impl InlineCommandBus {
    pub fn new() -> Self {
        Self { reached: Arc::new(AtomicBool::new(false)) }
    }

    pub fn was_reached(&self) -> bool {
        self.reached.load(Ordering::SeqCst)
    }
}

impl CommandBus for InlineCommandBus {
    fn dispatch<C: Command>(
        &self,
        _envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        self.reached.store(true, Ordering::SeqCst);
        async { Ok(()) }
    }
}

// ── AlwaysValidCommand ────────────────────────────────────────────────────────

/// Command that satisfies every constraint — `validate()` is the default no-op.
#[allow(dead_code)]
pub struct AlwaysValidCommand {
    pub value: String,
}

impl Validate for AlwaysValidCommand {}
impl Command for AlwaysValidCommand {}

// ── AlwaysInvalidCommand ──────────────────────────────────────────────────────

/// Command that always fails validation with a single `VAL-1001` violation on
/// the `username` field.
#[allow(dead_code)]
pub struct AlwaysInvalidCommand {
    pub username: String,
}

impl Validate for AlwaysInvalidCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        Err(vec![FieldViolation::new(
            "username",
            VAL_1001_REQUIRED,
            "must not be empty",
        )])
    }
}

impl Command for AlwaysInvalidCommand {}

// ── MultiViolationCommand ─────────────────────────────────────────────────────

/// Command that always fails validation with violations on three distinct
/// fields, exercising the full aggregation path.
pub struct MultiViolationCommand;

impl Validate for MultiViolationCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        Err(vec![
            FieldViolation::new("username", VAL_1001_REQUIRED, "must not be empty"),
            FieldViolation::new("bio",      VAL_1002_LENGTH,   "must be at most 160 characters"),
            FieldViolation::new("age",      VAL_1004_RANGE,    "must be at least 13"),
        ])
    }
}

impl Command for MultiViolationCommand {}
