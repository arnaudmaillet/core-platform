//! `search-api` — the generated gRPC contract for `search.v1` (server + client
//! stubs + descriptor), compiled from the shared `contracts/proto` IDL.
//! Consumers depend on this crate instead of recompiling the `.proto` files.
//!
//! Contract rule (the projection guarantee): the query surface is **read-only**
//! and **self-contained**. `Search` / `Suggest` / `MultiSearch` return ranked
//! references — `(entity_type, id, score, snippet)` plus the minimal indexed
//! display fields — never a hydrated, authoritative entity. There is **no write
//! RPC**: indexing happens entirely off the synchronous path via Kafka
//! consumers, and search publishes no events of its own (it is a terminal
//! read-model). The query path is fail-open: a partial/unavailable engine yields
//! `SearchResponse.degraded = true`, never an upstream block. Personal block/mute
//! is applied by the caller via `SearchRequest.exclude_author_ids`, never
//! indexed. See `project_search_service_blueprint`.
tonic::include_proto!("search.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("search_descriptor");
