//! Platform-wide input validation middleware and error mapping.
//!
//! Re-exports [`validate_core::Validate`] and [`validate_core::FieldViolation`]
//! as the canonical public surface so downstream crates need only one import.

pub mod error;
pub mod middleware;

pub use error::*;
pub use middleware::*;

pub use validate_core::{FieldViolation, Validate};
