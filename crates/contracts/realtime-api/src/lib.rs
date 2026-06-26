//! `realtime-api` — the generated contract for `realtime.v1` (server + client
//! stubs + descriptor), compiled from the shared `contracts/proto` IDL.
//! Consumers depend on this crate instead of recompiling the `.proto` files.
//!
//! This contract is unusual in the fleet: it has **two audiences**.
//!
//! * **Client-facing transport (not gRPC).** `ClientFrame` / `ServerFrame` are
//!   the multiplexed WSS envelope. Clients connect over WebSocket Secure and
//!   exchange these prost-encoded frames; one socket per device carries every
//!   channel class (`dm` / `notif` / `presence` / `counter` / `feed`) at once.
//!   The handshake authenticates the edge token ONCE; frames are never
//!   re-authenticated per message. `Event.payload` is opaque — the plane forwards
//!   upstream bytes verbatim and never inspects or stores them.
//!
//! * **Internal node-hop (gRPC).** `RealtimeDispatchService.DeliverToNode` is the
//!   dispatcher → owning-gateway-node delivery surface — the gRPC variant of the
//!   `NodeChannel` port, carrying a `DeliverEnvelope`. v1 wires this hop as Redis
//!   Pub/Sub (fire-and-forget, fail-open) carrying the same message; the service
//!   is defined so the fabric can swap without a contract change.
//!
//! Contract posture (the delivery guarantee): the realtime plane is a
//! System-of-Connection, never a System of Record. A delivery miss costs latency,
//! never data — durability lives in the owning SoRs (`chat` / `notification` /
//! `counter`) and clients resume from a `ChannelCursor` on reconnect. Its only
//! authorization is channel-key ownership (a connection may subscribe only to
//! channels scoped to its pinned identity). See `project_realtime_blueprint`.

tonic::include_proto!("realtime.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("realtime_descriptor");
