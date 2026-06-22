use std::collections::HashMap;

use async_trait::async_trait;

use crate::domain::value_object::{PostId, ProfileId, ReactionKind};
use crate::error::EngagementError;

/// Full engagement snapshot for a single post. All values sourced from Redis.
#[derive(Debug, Default)]
pub struct PostEngagementSnapshot {
    /// Weighted score per reaction kind. Only non-zero kinds are present.
    pub reaction_scores: HashMap<String, i64>,
    pub view_count:      i64,
    pub share_count:     i64,
    pub comment_count:   i64,
}

impl PostEngagementSnapshot {
    pub fn total_weighted_score(&self) -> i64 {
        self.reaction_scores.values().sum()
    }
}

/// Port for the Redis-primary atomic scoring layer.
///
/// All write methods are O(1) and involve a single Redis round-trip (Lua EVAL
/// or INCR). The hot path never touches ScyllaDB.
#[async_trait]
pub trait ScoreStore: Send + Sync + 'static {
    /// Atomically applies the Lua swap script:
    /// 1. Decrements the old reaction kind score (if one existed).
    /// 2. Writes the new kind+weight to the per-profile HASH.
    /// 3. Increments the new reaction kind score.
    ///
    /// Returns `Some((old_kind, old_weight))` if a previous reaction was
    /// replaced, or `None` if this is the first reaction.
    async fn atomic_upsert_reaction(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
        new_kind:   ReactionKind,
        new_weight: i64,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError>;

    /// Atomically removes a reaction via the Lua removal script.
    ///
    /// Returns `Some((kind, weight))` if a reaction existed, or `None` if
    /// the profile had no active reaction on this post.
    async fn atomic_remove_reaction(
        &self,
        post_id:    &PostId,
        profile_id: &ProfileId,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError>;

    /// Increments the view counter for `post_id` and marks it dirty for flush.
    async fn incr_view(&self, post_id: &PostId) -> Result<(), EngagementError>;

    /// Increments the share counter for `post_id` and marks it dirty for flush.
    async fn incr_share(&self, post_id: &PostId) -> Result<(), EngagementError>;

    /// Increments the comment counter for `post_id`.
    async fn incr_comment(&self, post_id: &PostId) -> Result<(), EngagementError>;

    /// Decrements the comment counter for `post_id`.
    async fn decr_comment(&self, post_id: &PostId) -> Result<(), EngagementError>;

    /// Reads the full engagement snapshot from Redis. Used by the query handler.
    async fn get_snapshot(&self, post_id: &PostId) -> Result<PostEngagementSnapshot, EngagementError>;
}
