//! gRPC ingress for the `moderation.v1` service.

pub mod handler;
pub mod server;

pub use handler::{proto, ModerationServiceHandler, ModerationServiceServer};
pub use server::FILE_DESCRIPTOR_SET;
