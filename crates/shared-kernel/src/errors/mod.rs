// crates/shared-kernel/src/errors/mod.rs

mod app_error;
mod context;
mod error;
mod error_code;
mod result;

pub use app_error::AppError;
pub use context::ErrorContext;
pub use error::DomainError;
pub use error_code::ErrorCode;
pub use result::{AppResult, Result};
