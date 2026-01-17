// crates/shared-kernel/src/errors/mod.rs

mod app_error;
mod context;
mod error_code;
mod result;
mod error;

pub use app_error::AppError;
pub use context::ErrorContext;
pub use error_code::ErrorCode;
pub use result::{Result, AppResult};
pub use error::DomainError;