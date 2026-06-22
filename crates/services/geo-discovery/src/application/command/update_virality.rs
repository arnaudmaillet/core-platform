use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{SpatialIndex, TileRepository};
use crate::domain::value_object::{H3Index, H3Resolution, PostId, ViralityScore};
use crate::error::GeoDiscoveryError;

/// Updates the virality score for a post across all data layers.
///
/// Triggered by `ScoreUpdaterWorker` after it resolves the post's canonical
/// tile indices from ScyllaDB. The worker reads the R7 index from the card row,
/// derives R5 via `parent()` and R9 via `parent()` inversion, then dispatches
/// this command with all three indices populated.
///
/// Update order:
///   1. ScyllaDB `map_post_cards.virality_score` — always written first.
///   2. Redis ZSETs at R5, R7, R9 — XX (update-only) semantics.
///      Posts evicted by Top-K or cold-tile pruning are not repopulated here;
///      ScyllaDB remains the authoritative cold-start source.
///
/// The Redis card key (`sg:geo:card:{post_id}`) is NOT updated. Card scores
/// carry acceptable eventual consistency (max staleness ≤ remaining card TTL)
/// to avoid a GET–deserialize–mutate–SET cycle on the hot score event stream.
pub struct UpdateViralityWithTilesCommand {
    pub post_id:     String,
    pub new_score:   f64,
    /// Raw H3 cell index (i64) at resolution 5.
    pub h3_index_r5: i64,
    /// Raw H3 cell index (i64) at resolution 7 (canonical tile).
    pub h3_index_r7: i64,
    /// Raw H3 cell index (i64) at resolution 9.
    pub h3_index_r9: i64,
}

impl Command for UpdateViralityWithTilesCommand {}

impl Validate for UpdateViralityWithTilesCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "GEO-VAL-020", "post_id must not be empty"));
        }
        if !self.new_score.is_finite() || self.new_score < 0.0 {
            v.push(FieldViolation::new("new_score", "GEO-VAL-021", "new_score must be a finite non-negative number"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct UpdateViralityWithTilesHandler<SI, TR> {
    pub spatial_index:   Arc<SI>,
    pub tile_repository: Arc<TR>,
}

impl<SI, TR> CommandHandler<UpdateViralityWithTilesCommand> for UpdateViralityWithTilesHandler<SI, TR>
where
    SI: SpatialIndex + 'static,
    TR: TileRepository + 'static,
{
    type Error = GeoDiscoveryError;

    async fn handle(&self, envelope: Envelope<UpdateViralityWithTilesCommand>) -> Result<(), GeoDiscoveryError> {
        let cmd = &envelope.payload;

        let post_id = PostId::try_from(cmd.post_id.as_str())?;
        let score   = ViralityScore::new(cmd.new_score)?;

        let idx_r5 = H3Index::from_i64(cmd.h3_index_r5)?;
        let idx_r7 = H3Index::from_i64(cmd.h3_index_r7)?;
        let idx_r9 = H3Index::from_i64(cmd.h3_index_r9)?;

        // ── 1. ScyllaDB (authoritative; used for cold-start ZSET reconstruction) ──
        self.tile_repository
            .update_card_score(&post_id, score.as_f32())
            .await?;

        // ── 2. Redis ZSETs (concurrent, XX — no insert if member absent) ──────
        let (u5, u7, u9) = tokio::join!(
            self.spatial_index.update_score(idx_r5, H3Resolution::R5, &post_id, score),
            self.spatial_index.update_score(idx_r7, H3Resolution::R7, &post_id, score),
            self.spatial_index.update_score(idx_r9, H3Resolution::R9, &post_id, score),
        );
        if let Err(e) = u5.and(u7).and(u9) {
            tracing::warn!(
                post_id   = %post_id,
                error     = %e,
                "Redis ZSET score update failed — ScyllaDB is authoritative"
            );
        }

        tracing::debug!(
            post_id   = %post_id,
            new_score = score.as_f64(),
            h3_r7     = idx_r7.as_u64(),
            "virality score updated"
        );

        Ok(())
    }
}
