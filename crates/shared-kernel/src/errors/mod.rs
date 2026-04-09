// crates/shared-kernel/src/errors/mod.rs

mod app_error;
mod context;
mod domain_error;
mod infrastructure_error;
mod error_code;
mod result;

pub use app_error::AppError;
pub use context::ErrorContext;
pub use domain_error::DomainError;
pub use infrastructure_error::InfrastructureError;
pub use error_code::ErrorCode;
pub use result::{AppResult, Result};
