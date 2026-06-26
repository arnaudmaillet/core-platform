//! `media-api` — the generated gRPC contract for `media.v1` (server + client
//! stubs + descriptor), compiled from the shared `contracts/proto` IDL.
//! Consumers depend on this crate instead of recompiling the `.proto` files.
//!
//! Contract rule (the load-bearing one, enforced at proto review): **the media
//! control plane never carries bytes.** Every `media.v1` message is a small
//! control message — an upload *ticket*, an asset's *metadata*, a *delivery* URL.
//! A JPEG or an MP4 must never appear inside a request or response: raw bytes flow
//! on a separate data plane (client ⇄ object storage ⇄ CDN) that the service
//! *authorizes and orchestrates* but never *transports*. Pre-signed direct-to-
//! object-store uploads (Plane A) and CDN/​signed-URL delivery resolution
//! (Plane C) are the two halves of that guarantee; the async transformation
//! pipeline (Plane B) is Kafka-driven and off the synchronous path. Events are
//! serde structs (`media.v1.events`), not proto. See
//! `project_media_service_blueprint`.
tonic::include_proto!("media.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("media_descriptor");
