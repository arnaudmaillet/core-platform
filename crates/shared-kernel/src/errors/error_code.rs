// crates/shared-kernel/src/errors/error_code.rs
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ValidationFailed,
    NotFound,
    AlreadyExists,
    ConcurrencyConflict,
    Unauthorized,
    Forbidden,
    InternalError,
    InfrastructureFailure,
    ServiceUnavailable,
}
