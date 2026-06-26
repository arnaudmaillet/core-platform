//! `counter-api` — the generated gRPC contract for `counter.v1` (server + client
//! stubs + descriptor), compiled from the shared `contracts/proto` IDL.
//! Consumers depend on this crate instead of recompiling the `.proto` files.
//!
//! Contract rule (the projection guarantee): the surface is **read-only** and
//! serves *magnitudes*, never identities. `BatchGetCounters` / `GetTrending` /
//! `GetTimeSeries` answer **"how many?"** for an entity reference — they never
//! answer **"who?"** or **"which ones?"** (that is the per-actor edge state owned
//! by `engagement` and `social-graph`). There is **no write/increment RPC**:
//! ingestion happens entirely off the synchronous path via Kafka consumers, and
//! the only thing this service publishes is the coarse `counter.v1.popularity`
//! ranking signal. The read path is fail-open: a degraded tier yields a
//! stale-but-served snapshot (`degraded = true`), never an upstream block. See
//! `project_counter_analytics_blueprint`.

tonic::include_proto!("counter.v1");

/// Encoded protobuf descriptor set for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("counter_descriptor");
