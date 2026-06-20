//! End-to-end example: how the `auth-service` microservice consumes the shared
//! `error` crate.
//!
//! It shows the full lifecycle:
//!   1. Define a domain error with `thiserror`.
//!   2. Implement `AppError` on it.
//!   3. Build a `DistributedError<AuthError>` with a request context.
//!   4. `.log()` it (structured tracing) and turn it into an `ApiErrorResponse`.
//!   5. Wire it into axum via `impl IntoResponse for DistributedError<AuthError>`.
//!
//! Run with: `cargo run -p error --example auth_service`

use axum::Json;
use axum::response::{IntoResponse, Response};
use error::{
    AppError, DistributedError, ErrorContext, IntoApiResponse, Severity, into_api_response,
};
use http::StatusCode;
use thiserror::Error;

// 1. The service defines its own error enum. The crate imposes nothing here.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("the provided session token has expired")]
    TokenExpired,

    #[error("the credentials are invalid")]
    InvalidCredentials,

    #[error("identity provider is temporarily unavailable")]
    UpstreamUnavailable,
}

// 2. Implement the shared contract. Only `error_code` + `http_status` are
//    mandatory; the rest is overridden where the defaults don't fit.
impl AppError for AuthError {
    fn error_code(&self) -> &'static str {
        match self {
            AuthError::TokenExpired => "AUTH_TOKEN_EXPIRED",
            AuthError::InvalidCredentials => "AUTH_INVALID_CREDENTIALS",
            AuthError::UpstreamUnavailable => "AUTH_UPSTREAM_UNAVAILABLE",
        }
    }

    fn http_status(&self) -> StatusCode {
        match self {
            AuthError::TokenExpired | AuthError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AuthError::UpstreamUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            AuthError::TokenExpired => Severity::Low,
            AuthError::InvalidCredentials => Severity::Medium,
            AuthError::UpstreamUnavailable => Severity::High,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(self, AuthError::UpstreamUnavailable)
    }

    fn category(&self) -> &'static str {
        "AUTH"
    }

    fn user_facing_message(&self) -> &'static str {
        match self {
            AuthError::TokenExpired => "Your session has expired, please sign in again.",
            AuthError::InvalidCredentials => "The email or password is incorrect.",
            AuthError::UpstreamUnavailable => "Sign-in is temporarily unavailable, try again.",
        }
    }
}

// 5. axum glue.
//
//    The orphan rule forbids `impl IntoResponse for DistributedError<AuthError>`
//    directly: `IntoResponse` and `DistributedError` are both foreign, and
//    `DistributedError` is not `#[fundamental]`, so parameterizing it with the
//    local `AuthError` is not enough to make the impl local. The idiomatic fix
//    is a one-line newtype the service owns and returns from its handlers.
pub struct AuthApiError(pub DistributedError<AuthError>);

impl From<DistributedError<AuthError>> for AuthApiError {
    fn from(err: DistributedError<AuthError>) -> Self {
        AuthApiError(err)
    }
}

impl IntoResponse for AuthApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        // Log first (internal trace/span ids included), then return only the
        // safe client body and the mapped status code.
        err.log();
        let status = err.error.http_status();
        let body = into_api_response(&err);
        (status, Json(body)).into_response()
    }
}

/// Simulated handler body. Returning `Result<_, AuthApiError>` lets axum turn
/// the error into a response automatically (with `?` on `DistributedError`).
///
/// `DistributedError` is intentionally a rich, by-value envelope (it carries
/// the full `ErrorContext`), so on hot error paths a service may box it. We
/// keep it unboxed here to stay readable; the concrete type is preserved
/// either way (never `Box<dyn Error>`).
#[allow(clippy::result_large_err)]
fn authenticate() -> Result<(), DistributedError<AuthError>> {
    let ctx = ErrorContext::new("auth-service")
        .with_trace("4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7")
        .with_meta("route", "POST /v1/sessions")
        .with_meta("user_id", "u_12345");

    Err(DistributedError::new(AuthError::TokenExpired, ctx))
}

fn main() {
    // NOTE: a real service installs a `tracing` subscriber in its bootstrap;
    // without one, `.log()` events are simply dropped. The example focuses on
    // the error plumbing, not on log rendering.
    match authenticate() {
        Ok(()) => println!("authenticated"),
        Err(err) => {
            // 3 + 4: log the distributed error and render the client payload.
            err.log();

            // Either via the free function...
            let body = into_api_response(&err);
            // ...or via the `IntoApiResponse` trait on the raw error + context:
            let _same = err.error.to_api_response(&err.context);

            let json = serde_json::to_string_pretty(&body)
                .unwrap_or_else(|_| "<serialization failed>".to_string());
            println!("HTTP {}\n{json}", err.error.http_status());

            // 5: the same error flowing through the axum newtype as a handler
            // would return it. `into_response` re-logs and builds the response.
            let response = AuthApiError::from(err).into_response();
            println!("axum status: {}", response.status());
        }
    }
}
