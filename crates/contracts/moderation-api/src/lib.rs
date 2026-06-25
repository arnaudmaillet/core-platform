//! Generated gRPC contract for `moderation.v1` (server + client stubs +
//! descriptor), compiled from the shared `contracts/proto` IDL. Consumers depend
//! on this crate instead of recompiling the `.proto` files.
//!
//! Contract rule (the normalization guarantee, mirroring `auth.v1`): the wire
//! surface exposes only normalized integrity types — a [`SubjectRef`]
//! (entity_type, entity_id, actor_id, surface), plus [`PolicyCategory`],
//! [`ActionType`], and case/appeal ids — never classifier-vendor or
//! content-internal fields. Moderation *decides*; it never re-exposes a content
//! service's private shape.
//!
//! The synchronous surface is deliberately minimal: the `Screen` gate (Plane C),
//! case/queue/appeal management (ops console), the DSA Statement-of-Reasons
//! export, and the discouraged internal enforcement read. Enforcement events
//! (Plane B) are NOT proto — they are serde structs published to
//! `moderation.v1.events`. See `project_moderation_service_blueprint`.
tonic::include_proto!("moderation.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("moderation_descriptor");
