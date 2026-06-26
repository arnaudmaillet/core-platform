//! The inbound decode layer: search-owned deserialization of the thin wire events,
//! distilled to the domain's [`SourceEvent`](crate::domain::SourceEvent) input
//! contract. Pure and unit-tested — the per-topic split and gRPC content hydration
//! are wired in Phase 5.

pub mod decoder;
pub mod wire;

pub use decoder::{
    ContentRef, Decoded, decode_moderation, decode_post, map_moderation, map_post,
};
pub use wire::{ModerationWireEvent, PostWireEvent};
