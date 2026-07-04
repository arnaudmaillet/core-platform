//! The media infrastructure layer — concrete adapters implementing the application
//! ports against real backends.
//!
//! * **`persistence`** — the Postgres asset-metadata System of Record.
//! * **`cache`** — the Redis hot-path delivery cache.
//! * **`store`** — the S3/MinIO object store (pre-signed uploads + server-side
//!   byte I/O for the pipeline). The byte plane.
//! * **`cdn`** — content-addressed public URLs + signed private URLs.
//! * **`probe`** / **`processor`** — the image pipeline (decode/validate; resize
//!   ladder + BlurHash).
//! * **`scanner`** — the malware-scan stub (real sidecar slots in behind the port).
//! * **`screen`** — the `moderation` Screen gRPC client (fail-closed).
//! * **`event`** — the Kafka / log event publishers for `media.v1.events`.
//!
//! The gRPC service handler/server and the inbound consumers (finalize / moderation
//! / orphan-GC) are wired in Phase 5.

pub mod cache;
pub mod cdn;
pub mod consumer;
pub mod event;
pub mod grpc;
pub mod persistence;
pub mod probe;
pub mod processor;
pub mod scanner;
pub mod screen;
pub mod store;
