//! Framework-agnostic, client-facing error payload.
//!
//! [`ApiErrorResponse`] is the *only* shape that ever reaches a client. It is
//! deliberately built from an [`AppError`] + [`ErrorContext`] while dropping
//! anything internal (trace/span ids stay in the logs). This module knows
//! nothing about axum/actix/tonic: it produces a plain serializable struct and
//! a [`StatusCode`]. The web-framework glue lives in each service (see the
//! commented axum example below and the `examples/auth_service.rs` example).
//!
//! [`StatusCode`]: http::StatusCode

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::context::{DistributedError, ErrorContext};
use crate::severity::Severity;
use crate::traits::AppError;

/// JSON error body returned to API clients.
///
/// Serializes to, e.g.:
/// ```json
/// {
///   "error_code": "AUTH_TOKEN_EXPIRED",
///   "message": "Your session has expired, please sign in again.",
///   "request_id": "550e8400-e29b-41d4-a716-446655440000",
///   "service": "auth-service",
///   "severity": "High",
///   "retryable": false,
///   "category": "AUTH",
///   "timestamp": "2024-01-15T10:30:00Z",
///   "details": {}
/// }
/// ```
///
/// Note the absence of `trace_id`/`span_id`: those are internal correlation
/// identifiers and must never leak to clients. The `request_id` is safe to
/// expose and is what a user quotes to support.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// Stable machine-readable code, from [`AppError::error_code`].
    pub error_code: String,
    /// Safe end-user message, from [`AppError::user_facing_message`].
    pub message: String,
    /// Correlation id the client can quote to support.
    pub request_id: Uuid,
    /// Emitting microservice.
    pub service: String,
    /// Operational severity.
    pub severity: Severity,
    /// Whether retrying is safe.
    pub retryable: bool,
    /// Coarse error category.
    pub category: String,
    /// When the error occurred (UTC, RFC 3339 on the wire).
    pub timestamp: DateTime<Utc>,
    /// Arbitrary public details, mirrored from the context metadata.
    pub details: HashMap<String, String>,
}

impl ApiErrorResponse {
    /// Single source of truth for building the response from an error and its
    /// context. Used by both [`into_api_response`] and the
    /// [`IntoApiResponse`](crate::IntoApiResponse) trait.
    pub fn from_error<E: AppError + ?Sized>(error: &E, context: &ErrorContext) -> Self {
        Self {
            error_code: error.error_code().to_string(),
            message: error.user_facing_message().to_string(),
            request_id: context.request_id,
            service: context.service_name.to_string(),
            severity: error.severity(),
            retryable: error.is_retryable(),
            category: error.category().to_string(),
            timestamp: context.timestamp,
            details: context.metadata.clone(),
        }
    }
}

/// Builds the client-facing [`ApiErrorResponse`] from a [`DistributedError`],
/// independently of any HTTP framework.
///
/// Pair the returned body with the [`StatusCode`] from
/// [`AppError::http_status`] in your framework's response type.
pub fn into_api_response<E: AppError>(err: &DistributedError<E>) -> ApiErrorResponse {
    ApiErrorResponse::from_error(&err.error, &err.context)
}

// ---------------------------------------------------------------------------
// Example: integrating with axum (kept as a doc comment so this crate stays
// framework-agnostic — no `axum` in its runtime dependencies).
//
// The orphan rule forbids `impl IntoResponse for DistributedError<MyError>`:
// both `IntoResponse` and `DistributedError` are foreign, and `DistributedError`
// is not `#[fundamental]`, so wrapping a local error in it does not make the
// impl local. Each service therefore owns a one-line newtype and implements
// `IntoResponse` on that. A full, compiling version lives in
// `examples/auth_service.rs`.
//
// ```ignore
// use axum::response::{IntoResponse, Response};
// use axum::Json;
// use error::{into_api_response, AppError, DistributedError};
//
// // Newtype owned by the service; `From` lets `?` convert errors in handlers.
// pub struct ApiError(pub DistributedError<MyServiceError>);
//
// impl From<DistributedError<MyServiceError>> for ApiError {
//     fn from(err: DistributedError<MyServiceError>) -> Self { ApiError(err) }
// }
//
// impl IntoResponse for ApiError {
//     fn into_response(self) -> Response {
//         let err = self.0;
//         // Emit the structured, context-rich log (with trace/span ids)...
//         err.log();
//         // ...then return only the safe, client-facing body.
//         let status = err.error.http_status();
//         let body = into_api_response(&err);
//         (status, Json(body)).into_response()
//     }
// }
// ```
// ---------------------------------------------------------------------------
