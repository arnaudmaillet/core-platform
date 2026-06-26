//! The `media.v1` gRPC ingress — the request handler (proto ↔ application mapping)
//! and the generated-trait impl (`server`).

pub mod handler;
pub mod server;

pub use handler::{proto, MediaServiceHandler};
pub use proto::media_service_server::MediaServiceServer;
pub use server::FILE_DESCRIPTOR_SET;
