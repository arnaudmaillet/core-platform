use async_trait::async_trait;

use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;
use crate::infrastructure::persistence::model::ReactionRow;

/// Port for the ScyllaDB durable reaction ledger.
///
/// Write operations are called exclusively from background workers (not on the
/// gRPC hot path). The ledger is the source of truth for Redis cold-start recovery.
#[async_trait]
pub trait ReactionLedger: Send + Sync + 'static {
    /// Upserts a reaction record. Last-write-wins (no IF conditions) — safe
    /// to retry on Kafka redelivery.
    async fn upsert(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
        kind:       ReactionKind,
        weight:     i64,
        event_at_ms: i64,
    ) -> Result<(), EngagementError>;

    /// Deletes the reaction record for `(post_id, profile_id)`.
    async fn remove(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
    ) -> Result<(), EngagementError>;

    /// Scans all reactions for `post_id`. Used during cold-start Redis reconstruction.
    async fn scan_for_recovery(
        &self,
        post_id: &PostId,
    ) -> Result<Vec<ReactionRow>, EngagementError>;

    /// Applies a view/share/comment counter delta to the ScyllaDB counter table.
    async fn apply_interaction_delta(
        &self,
        post_id:       &PostId,
        view_delta:    i64,
        share_delta:   i64,
        comment_delta: i64,
    ) -> Result<(), EngagementError>;
}
