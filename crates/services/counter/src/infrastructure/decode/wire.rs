//! Counter-owned deserialization DTOs for the events it consumes.
//!
//! Counter must not depend on the `engagement` / `social-graph` crates (a sideways
//! services→services edge the tiering forbids), so it owns its read schema:
//! minimal, lenient structs that match the published JSON. Extra fields are
//! ignored, so an additive change upstream never breaks a consumer.
//!
//! Integration reality (mirrors `search`'s honesty about thin events):
//! * `view` / `impression` / `click` are **counter-owned firehose schemas** — no
//!   upstream producer exists yet; the edge/BFF will produce telemetry matching
//!   these shapes. They are notifications, and counts need nothing more than the
//!   `(entity, actor?, time)` they carry — no hydration.
//! * `engagement.reactions` **matches the live upstream schema** (`engagement`
//!   publishes it today, internally tagged on `event_type`, snake_case).
//! * `social-graph` follow events are a **counter-owned schema** pending an
//!   upstream follow stream (an upstream prerequisite, like `profile.v1.events`
//!   is for search).

use serde::Deserialize;

// ── view / impression / click — counter-owned firehose schema ─────────────────

/// One engagement hit on an entity. `actor_id`, when present, is folded into the
/// unique-cardinality estimator (unique viewers / reach); it is never stored.
#[derive(Debug, Clone, Deserialize)]
pub struct HitWire {
    pub entity_type: String,
    pub entity_id: String,
    #[serde(default)]
    pub actor_id: Option<String>,
    pub occurred_at_ms: i64,
}

// ── engagement.reactions — MATCHES the upstream engagement schema ─────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum ReactionWire {
    Upserted(ReactionUpsertedWire),
    Removed(ReactionRemovedWire),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionUpsertedWire {
    pub post_id: String,
    /// Present ⇒ this upsert *replaced* a prior reaction, so the reaction count is
    /// unchanged (net-zero like delta). Absent ⇒ a brand-new reaction (`+1`).
    #[serde(default)]
    pub old_kind: Option<String>,
    pub event_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReactionRemovedWire {
    pub post_id: String,
    pub event_at_ms: i64,
}

// ── social-graph follow — counter-owned schema (upstream stream is a prereq) ──

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FollowWire {
    Followed(FollowChangeWire),
    Unfollowed(FollowChangeWire),
}

#[derive(Debug, Clone, Deserialize)]
pub struct FollowChangeWire {
    pub follower_id: String,
    pub followee_id: String,
    pub occurred_at_ms: i64,
}
