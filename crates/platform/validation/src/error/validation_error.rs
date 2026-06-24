//! [`ValidationError`] — the concrete, [`AppError`]-implementing error type
//! produced when a command fails its [`Validate`] contract.
//!
//! ## VAL-xxxx code catalogue
//!
//! | Code     | Constraint violated                                      |
//! |----------|----------------------------------------------------------|
//! | VAL-1001 | Required field is absent or empty                        |
//! | VAL-1002 | Value length out of bounds (min / max)                   |
//! | VAL-1003 | Value does not match the expected regex pattern          |
//! | VAL-1004 | Numeric value out of the allowed range                   |
//! | VAL-1005 | Malformed e-mail address                                 |
//! | VAL-1006 | Malformed URL                                            |
//! | VAL-1007 | Enum variant unknown or not permitted in this context    |
//! | VAL-1008 | Collection size out of bounds (min / max items)          |
//! | VAL-1009 | Duplicate value where uniqueness is required             |
//! | VAL-9000 | Catch-all: constraint not covered by the codes above     |

use std::collections::HashMap;
use std::fmt;

use http::StatusCode;

use error::{AppError, Severity};
use validate_core::FieldViolation;

// ── VAL-xxxx public constants ─────────────────────────────────────────────────

/// Required field is absent or empty.
pub const VAL_1001_REQUIRED: &str = "VAL-1001";
/// Value length out of bounds (min / max characters or bytes).
pub const VAL_1002_LENGTH: &str = "VAL-1002";
/// Value does not match the expected regex / format pattern.
pub const VAL_1003_PATTERN: &str = "VAL-1003";
/// Numeric value out of the allowed range.
pub const VAL_1004_RANGE: &str = "VAL-1004";
/// Malformed e-mail address.
pub const VAL_1005_EMAIL: &str = "VAL-1005";
/// Malformed URL.
pub const VAL_1006_URL: &str = "VAL-1006";
/// Enum variant unknown or not permitted in this context.
pub const VAL_1007_ENUM: &str = "VAL-1007";
/// Collection size out of bounds (min / max items).
pub const VAL_1008_SIZE: &str = "VAL-1008";
/// Duplicate value where uniqueness is required.
pub const VAL_1009_UNIQUE: &str = "VAL-1009";
/// Catch-all for constraints not covered by the codes above.
pub const VAL_9000_CUSTOM: &str = "VAL-9000";

// ── ValidationError ───────────────────────────────────────────────────────────

/// Aggregated error produced when one or more [`FieldViolation`]s are found
/// during command validation.
///
/// Implements [`AppError`] so it can be wrapped in [`CqrsError::Handler`] and
/// surfaced through the normal error pipeline without special-casing.
///
/// ## HTTP contract
///
/// Always maps to **422 Unprocessable Entity** with [`Severity::Low`]:
/// validation failures are client errors, not service failures.
///
/// ## Client-facing shape (via `ApiErrorResponse.details`)
///
/// Each violation is serialised into the `details` map as:
/// ```text
/// "<field>" → "<code>: <message>"
/// ```
/// For example:
/// ```json
/// {
///   "error_code": "VAL-0001",
///   "details": {
///     "username": "VAL-1001: must not be empty",
///     "age":      "VAL-1004: must be at least 13"
///   }
/// }
/// ```
#[derive(Debug)]
pub struct ValidationError {
    violations: Vec<FieldViolation>,
}

impl ValidationError {
    /// Wraps a non-empty list of [`FieldViolation`]s.
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `violations` is empty — a `ValidationError`
    /// with no violations is a programming error.
    pub fn new(violations: Vec<FieldViolation>) -> Self {
        debug_assert!(!violations.is_empty(), "ValidationError must contain at least one violation");
        Self { violations }
    }

    /// Returns a reference to the collected violations.
    pub fn violations(&self) -> &[FieldViolation] {
        &self.violations
    }

    /// Flattens violations into a `field → "code: message"` map suitable for
    /// embedding in [`ApiErrorResponse::details`](error::ApiErrorResponse).
    ///
    /// When multiple violations hit the same field the last one wins; prefer
    /// surfacing all violations via [`violations()`](Self::violations) in
    /// structured contexts.
    pub fn to_details_map(&self) -> HashMap<String, String> {
        self.violations
            .iter()
            .map(|v| (v.field.to_string(), format!("{}: {}", v.code, v.message)))
            .collect()
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "validation failed ({} violation(s)):", self.violations.len())?;
        for v in &self.violations {
            write!(f, " [{field}] {code} — {msg};",
                field = v.field,
                code  = v.code,
                msg   = v.message,
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ValidationError {}

impl AppError for ValidationError {
    fn error_code(&self) -> &'static str {
        "VAL-0001"
    }

    fn http_status(&self) -> StatusCode {
        StatusCode::UNPROCESSABLE_ENTITY
    }

    fn severity(&self) -> Severity {
        Severity::Low
    }

    fn is_retryable(&self) -> bool {
        false
    }

    fn category(&self) -> &'static str {
        "VALIDATION"
    }

    fn user_facing_message(&self) -> &'static str {
        "One or more fields failed validation. Please review and correct your input."
    }
}
