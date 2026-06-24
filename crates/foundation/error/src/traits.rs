//! The core [`AppError`] contract every microservice error enum implements.
//!
//! This module defines *behaviour*, never concrete errors. A service declares
//! its own `enum SomethingError` (typically with `thiserror`) and implements
//! [`AppError`] to describe how that error should be coded, ranked, retried and
//! surfaced. The shared infrastructure ([`DistributedError`], HTTP mapping,
//! logging) is written purely against this trait, so it works for any service
//! without ever depending on a specific domain.

use http::StatusCode;

use crate::context::ErrorContext;
use crate::http::ApiErrorResponse;
use crate::severity::Severity;

/// Contract for a service-defined error.
///
/// Implementors must be real [`std::error::Error`]s and `Send + Sync +
/// 'static` so they can cross thread and task boundaries and be stored in
/// long-lived telemetry without lifetime gymnastics.
///
/// Only [`AppError::error_code`] and [`AppError::http_status`] are mandatory;
/// every other method has a conservative default so a brand-new error enum is
/// useful with minimal boilerplate, then progressively refined.
pub trait AppError: std::error::Error + Send + Sync + 'static {
    /// Stable, machine-readable identifier, e.g. `"AUTH_TOKEN_EXPIRED"`.
    ///
    /// This is part of your public API contract: clients and dashboards key off
    /// it, so treat changes as breaking.
    fn error_code(&self) -> &'static str;

    /// HTTP status the gateway should return for this error.
    fn http_status(&self) -> StatusCode;

    /// Operational severity used for alerting and log levels. Defaults to
    /// [`Severity::Medium`].
    fn severity(&self) -> Severity {
        Severity::Medium
    }

    /// Whether the caller may safely retry the operation. Defaults to `false`;
    /// override for transient failures (timeouts, contention, upstream 503s).
    fn is_retryable(&self) -> bool {
        false
    }

    /// Coarse grouping for metrics, e.g. `"AUTH"`, `"DB"`, `"RATE_LIMIT"`.
    fn category(&self) -> &'static str {
        "UNKNOWN"
    }

    /// Safe, non-leaking message intended for the end user. Defaults to a
    /// generic string so internal details are never exposed by accident.
    fn user_facing_message(&self) -> &'static str {
        "An error occurred."
    }
}

/// Bridges an [`AppError`] and an [`ErrorContext`] into the client-facing
/// [`ApiErrorResponse`].
///
/// It is automatically implemented for every [`AppError`] via a blanket impl,
/// so any service error gains `.to_api_response(&ctx)` for free. The default
/// method delegates to [`ApiErrorResponse::from_error`], which is the single
/// source of truth for the response shape.
pub trait IntoApiResponse: AppError + Sized {
    /// Combines `self` with the distributed `ctx` to produce the serializable
    /// response. Internal-only fields of the context (trace/span ids) are
    /// deliberately dropped here — see [`ApiErrorResponse`].
    fn to_api_response(&self, ctx: &ErrorContext) -> ApiErrorResponse {
        ApiErrorResponse::from_error(self, ctx)
    }
}

impl<E: AppError> IntoApiResponse for E {}
