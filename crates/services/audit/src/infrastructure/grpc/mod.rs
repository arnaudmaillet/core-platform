//! gRPC ingress for the `audit.v1` service — the read/record surface.

pub mod handler;
pub mod server;

pub use handler::{AuditServiceHandler, proto};
pub use proto::audit_service_server::AuditServiceServer;
pub use server::FILE_DESCRIPTOR_SET;
