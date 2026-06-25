//! `moderation` — the platform's integrity **decision-of-record and enforcement
//! brain** (trust, safety & compliance).
//!
//! This service owns the *decision*: what action is taken against what entity,
//! under which policy version, with what evidence — and it is the authoritative,
//! auditable source of that fact for the rest of the fleet and for regulators.
//! It is deliberately distinct from its neighbours:
//!
//! * the **content services** (`post` / `comment` / `chat` / `media`) own the
//!   content bytes and its visibility; they *react* to enforcement, moderation
//!   never stores a second copy.
//! * `account` is the identity System of Record and owns suspension/ban
//!   *execution*; moderation *decides* and emits the enforcement event.
//! * the **classifier services** own ML inference + hash corpora; moderation
//!   *consumes* their signals, it never runs inference inline.
//! * `social-graph` owns personal block/mute (a safety preference, not platform
//!   integrity).
//!
//! The architectural commitment is the separation of the heavy
//! classification/review path from the hot decision path — three planes:
//! **(A)** async post-hoc ingestion (the default, ~99% of content),
//! **(B)** denormalized enforcement state on the hot *read* path (events + a
//! Redis projection, never a per-item RPC), and **(C)** a narrow, fail-closed
//! synchronous `Screen` gate for catastrophic-harm categories only. See
//! `project_moderation_service_blueprint` for the full design.
//!
//! ## Module roadmap (built phase by phase)
//! Phase 0 (now): [`error`] — the canonical `MOD-XXXX` namespace.
//! Phase 2: `domain` · Phase 3: `application` + ports · Phase 4: `infrastructure`
//! · Phase 5: `app` (composition root) + `service` (runtime wiring).

pub mod app;
pub mod application;
pub mod config;
pub mod domain;
pub mod error;
pub mod infrastructure;
pub mod service;

pub use error::ModerationError;
