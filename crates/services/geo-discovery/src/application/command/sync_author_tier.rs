use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::port::{CardStore, TileRepository};
use crate::domain::value_object::{AuthorTier, PostId};
use crate::error::GeoDiscoveryError;

/// Updates the `author_tier` projection for a single post.
///
/// Triggered by `TierSyncWorker` on every `profile.tier_changed` Kafka event.
/// `services/profile` emits one event per affected post_id so this handler
/// remains stateless and requires no author→posts index.
///
/// Write order:
///   1. ScyllaDB UPDATE (durable, authoritative).
///   2. Redis card key DELETE (invalidate stale msgpack; next read re-caches
///      with the corrected tier from ScyllaDB).
pub struct SyncAuthorTierCommand {
    /// Hyphenated UUID string of the post to update.
    pub post_id:  String,
    /// New tier value. 0=Standard, 1=Premium, 2=VIP.
    pub new_tier: u8,
}

impl Command for SyncAuthorTierCommand {}

impl Validate for SyncAuthorTierCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.post_id.trim().is_empty() {
            v.push(FieldViolation::new("post_id", "GEO-VAL-006", "post_id must not be empty"));
        }
        if self.new_tier > 2 {
            v.push(FieldViolation::new("new_tier", "GEO-VAL-007", "new_tier must be 0, 1, or 2"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct SyncAuthorTierHandler<CS, TR> {
    pub card_store:      Arc<CS>,
    pub tile_repository: Arc<TR>,
}

impl<CS, TR> CommandHandler<SyncAuthorTierCommand> for SyncAuthorTierHandler<CS, TR>
where
    CS: CardStore + 'static,
    TR: TileRepository + 'static,
{
    type Error = GeoDiscoveryError;

    async fn handle(
        &self,
        envelope: Envelope<SyncAuthorTierCommand>,
    ) -> Result<(), GeoDiscoveryError> {
        let cmd  = &envelope.payload;
        let post_id = PostId::try_from(cmd.post_id.as_str())?;
        let tier    = AuthorTier::from_u8(cmd.new_tier);

        // 1. Durable update in ScyllaDB (Strict profile, LocalQuorum).
        self.tile_repository
            .update_card_tier(&post_id, tier.as_i8())
            .await?;

        // 2. Invalidate Redis card so the next read picks up the new tier.
        if let Err(e) = self.card_store.del(&post_id).await {
            tracing::warn!(
                post_id = %post_id,
                error   = %e,
                "card cache invalidation failed after tier sync — ScyllaDB is authoritative"
            );
        }

        tracing::debug!(
            post_id  = %post_id,
            new_tier = cmd.new_tier,
            "author tier synced"
        );

        Ok(())
    }
}
