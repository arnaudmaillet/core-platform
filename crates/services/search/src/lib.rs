//! `search` — the platform's **discovery read-model**: a derived, typo-tolerant
//! inverted index over profiles, posts, and hashtags.
//!
//! This service is a System-of-*Reference*, never a System-of-Record. Every byte
//! in its index is a disposable copy that must be reconstructable from the source
//! services at any moment. That single framing sets it apart from its neighbours:
//!
//! * the **content/identity services** (`post` / `profile` / `comment`) own the
//!   authoritative entity; search holds only the minimal projection needed to
//!   *match*, *rank*, and render a result row — it resolves nothing on the fly.
//! * `moderation` owns the integrity decision; search *consumes*
//!   `moderation.v1.events` to flip a document's `searchable` flag (a global
//!   visibility input), and reverses it on appeal.
//! * `social-graph` owns personal block/mute; that is a **per-viewer** filter
//!   applied at the edge, never baked into the shared index.
//! * `engagement` owns real-time counts; search folds in only a **coarse,
//!   periodic** popularity signal for ranking — never a per-event count.
//!
//! The architectural commitment is the cleanest CQRS split in the fleet: the
//! **command side is 100% Kafka consumers** (there is no write RPC at all), and
//! the **query side is a stateless gRPC read** that makes no inter-service call
//! on the hot path. Posture is the inverse of `moderation`: search is
//! **best-effort, fail-open, eventually-consistent** — a search outage degrades
//! discovery, it never blocks a write, a publish, or a login. See
//! `project_search_service_blueprint` for the full design.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `SCH-XXXX` namespace.
//! Phase 2: `domain` (IndexDocument + per-entity projections + the pure
//! projector) · Phase 3: `application` + ports · Phase 4: `infrastructure`
//! (OpenSearch adapter + event-decode) · Phase 5: `app` (composition root) +
//! `service` (runtime wiring + self-spawned ingestion consumers).

pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;

pub use error::SearchError;
