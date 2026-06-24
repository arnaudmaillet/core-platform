//! Generated gRPC contract for `social_graph.v1`.
//!
//! Exposes both server (`social_graph_service_server`) and client
//! (`social_graph_service_client`) stubs plus the message types, generated from
//! the shared `contracts/proto` IDL at build time. Consumers depend on this crate
//! instead of recompiling the `.proto` files themselves:
//!
//! - the social-graph service implements the server stubs;
//! - timeline (and any future caller) uses the client stubs — no cross-service
//!   proto recompilation.
tonic::include_proto!("social_graph.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("social_graph_descriptor");
