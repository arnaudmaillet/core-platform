//! Shared error infrastructure for the workspace.
//!
//! This crate has a single responsibility: provide the common building blocks
//! (traits, distributed context, serialization format) that let every
//! microservice define its *own* error enums in a standardized, decoupled and
//! observable way. It contains **no business logic and no domain** — it does
//! not know auth, users or databases, and it does not know its consumers.
//!
//! ## The four pieces
//!
//! - [`AppError`] / [`IntoApiResponse`] ([`traits`]) — the contract a service
//!   implements on its error enum.
//! - [`Severity`] ([`severity`]) — shared urgency vocabulary driving paging and
//!   log levels.
//! - [`ErrorContext`] / [`DistributedError`] ([`context`]) — request/trace
//!   metadata and the type-preserving error envelope.
//! - [`ApiErrorResponse`] / [`into_api_response`] ([`http`]) — the
//!   framework-agnostic, client-facing JSON payload.
//!
//! A runnable end-to-end usage (auth-service + axum) lives in
//! `examples/auth_service.rs`.

pub mod context;
pub mod http;
pub mod severity;
pub mod traits;

pub use context::{DistributedError, ErrorContext};
pub use http::{ApiErrorResponse, into_api_response};
pub use severity::Severity;
pub use traits::{AppError, IntoApiResponse};
