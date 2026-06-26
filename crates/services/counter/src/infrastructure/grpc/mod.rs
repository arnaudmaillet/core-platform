//! gRPC ingress for the `counter.v1` service (read-only).

pub mod handler;
pub mod server;

pub use handler::{CounterServiceHandler, proto};
pub use proto::counter_service_server::CounterServiceServer;
pub use server::FILE_DESCRIPTOR_SET;
