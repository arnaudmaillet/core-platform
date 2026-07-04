//! gRPC ingress for the `search.v1` service.

pub mod handler;
pub mod server;

pub use handler::{SearchServiceHandler, proto};
pub use proto::search_service_server::SearchServiceServer;
pub use server::FILE_DESCRIPTOR_SET;
