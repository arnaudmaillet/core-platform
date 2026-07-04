//! Live, container-backed integration suite for the media service.
//!
//! Media's whole point is the byte plane, so this suite boots a real **MinIO**
//! (S3-compatible object storage) alongside **Postgres** (the metadata SoR) and
//! **Redis** (the delivery cache), and drives the production handlers over the
//! real adapters — the S3 presign + server-side I/O, the `image`-crate probe and
//! resize/BlurHash pipeline, and the Postgres/Redis stores. It exercises exactly
//! what cannot be unit-tested: the client uploading bytes to a pre-signed URL, the
//! pipeline reading them back, decoding, deriving renditions, and writing them to
//! content-addressed keys.
//!
//! The moderation Screen and the malware scanner are stubbed at their port
//! boundary (there is no live `moderation` here); everything else is real.
//!
//! Gated behind `integration-media` so the default `cargo test -p media` stays
//! hermetic and Docker-free. Run the live suite:
//!
//! ```text
//! cargo test -p media --features integration-media -- --nocapture
//! ```
//!
//! Coverage:
//! - **pipeline** — ticket → direct-to-MinIO PUT → commit → process → READY, with
//!   real renditions present in the store and resolvable public URLs; plus
//!   content-hash dedup.
//! - **moderation** — a CSAM screen block quarantines the asset, places a legal
//!   hold, and that hold blocks deletion; a takedown then restore round-trips.
//! - **validation** — non-image bytes are rejected by the real probe; an oversize
//!   declaration is rejected at ticket time.
#![cfg(feature = "integration-media")]

mod media_it;
