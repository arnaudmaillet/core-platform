//! `media` — the platform's **media control plane**: the system-of-record for
//! media *assets and their processing lifecycle*, and the broker of the byte
//! plane that surrounds them.
//!
//! The single framing that sets this service apart from every neighbour: **it is
//! a control plane, not a data plane.** Bytes never traverse gRPC and never
//! traverse Kafka. What moves through the mesh is tiny — upload *tickets*, asset
//! *metadata*, CDN *URLs*; the bytes themselves flow on a separate path (client ⇄
//! object storage ⇄ CDN) that this service *authorizes and orchestrates* but
//! never *carries*. If a JPEG or an MP4 ever shows up inside a `tonic` request or
//! a Kafka record, the design has failed.
//!
//! That posture places it next to its neighbours like so:
//!
//! * the **content/identity services** (`post` / `profile`) store an `asset_id`
//!   *reference*, never bytes, and never wait on processing — a post publishes
//!   referencing a still-processing asset, rendering a blurhash placeholder until
//!   `AssetReady` lands. Media is upstream of, and decoupled from, the core write
//!   path.
//! * `moderation` owns the integrity *decision*; media computes perceptual/crypto
//!   hashes and calls `Screen` before a CSAM-class asset can go public
//!   (fail-closed), then *enforces* quarantine on the byte plane — the reactive
//!   visibility flip moderation delegates to content services.
//! * the **CDN** owns edge fan-out; media owns the *delivery policy* —
//!   content-addressed immutable URLs for public media (an edit is a new asset =
//!   a new hash = a new URL, so cache invalidation is a takedown-only operation)
//!   and short-lived signed URLs for private media.
//!
//! Failure posture is **mixed**, and the README fail matrix splits it: the
//! *delivery* plane is fail-**open** (a 404 avatar degrades UX, it never blocks a
//! write or a login), while the narrow *compliance* gate is fail-**closed**
//! (CSAM-class media must never go public on uncertainty or a Screen outage).
//! Processing lag is an SLO, not a consistency requirement — the publish path
//! never waits on a transcode. See `project_media_service_blueprint`.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `MED-XXXX` namespace.
//! Phase 2: `domain` (the Asset aggregate + lifecycle state machine, Rendition
//! catalog, UploadTicket, content-addressed value objects, the pure transition
//! logic) · Phase 3: `application` + async ports · Phase 4: `infrastructure`
//! (S3/MinIO object-store adapter, Postgres metadata SoR, Redis delivery cache,
//! image/transcode processors, CDN signer, inbound finalize/moderation consumers)
//! · Phase 5: `app` (composition root) + `service` (runtime wiring + the
//! self-spawned processing & compliance consumers).

pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;

pub use error::MediaError;
pub use service::{MediaService, MediaWorkerService};
