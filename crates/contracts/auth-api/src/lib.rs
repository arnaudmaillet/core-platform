//! Generated gRPC contract for `auth.v1` (server + client stubs + descriptor),
//! compiled from the shared contracts/proto IDL. Consumers depend on this crate
//! instead of recompiling the .proto files.
//!
//! Contract rule (the IdP-agnosticism guarantee): `auth.v1` exposes only
//! normalized identity types (`account_id`, `session_id`, `generation`,
//! `permissions`) — never Keycloak/IdP-specific fields. See
//! `project_auth_service_blueprint`.
tonic::include_proto!("auth.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("auth_descriptor");
