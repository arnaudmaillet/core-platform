use std::sync::Arc;

use chrono::{DateTime, Utc};
use cqrs::Envelope;

use crate::application::port::{AssetRepository, CdnGateway, DeliveryCache, EventPublisher};
use crate::domain::value_object::{AssetId, AssetState, StorageKey};
use crate::error::MediaError;

/// The takedown direction, distilled from a consumed `moderation.v1.events`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationAction {
    Quarantine,
    Restore,
}

/// Apply a moderation decision to an asset (driven by the moderation consumer,
/// Phase 5).
#[derive(Debug, Clone)]
pub struct ApplyModerationCommand {
    pub asset_id: AssetId,
    pub action: ModerationAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyModerationOutcome {
    /// `false` when the asset is unknown (the event is for media we don't hold) —
    /// a folded no-op the consumer commits.
    pub applied: bool,
    pub state: Option<AssetState>,
}

/// Reactively enforces a moderation verdict on the byte plane: a quarantine revokes
/// delivery (state flip + CDN invalidate + cache drop); a restore reinstates it.
/// This is the content-service side of moderation's enforcement — `media` flips
/// visibility, it does not decide.
pub struct ApplyModerationHandler {
    assets: Arc<dyn AssetRepository>,
    cdn: Arc<dyn CdnGateway>,
    cache: Arc<dyn DeliveryCache>,
    publisher: Arc<dyn EventPublisher>,
}

impl ApplyModerationHandler {
    pub fn new(
        assets: Arc<dyn AssetRepository>,
        cdn: Arc<dyn CdnGateway>,
        cache: Arc<dyn DeliveryCache>,
        publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self { assets, cdn, cache, publisher }
    }

    pub async fn handle(
        &self,
        envelope: Envelope<ApplyModerationCommand>,
        now: DateTime<Utc>,
    ) -> Result<ApplyModerationOutcome, MediaError> {
        let cmd = envelope.payload;
        let Some(mut asset) = self.assets.find_by_id(&cmd.asset_id).await? else {
            // Unknown asset — fold to a no-op so the consumer commits.
            return Ok(ApplyModerationOutcome { applied: false, state: None });
        };

        match cmd.action {
            ModerationAction::Quarantine => {
                asset.quarantine(now)?;
                // Revoke delivery for every rendition (the takedown path).
                let keys: Vec<StorageKey> =
                    asset.renditions().iter().map(|r| r.storage_key().clone()).collect();
                if !keys.is_empty() {
                    self.cdn.invalidate(&keys).await?;
                }
                self.cache.invalidate(&asset.id()).await?;
            }
            ModerationAction::Restore => {
                asset.restore(now)?;
                // Drop the (placeholder) cache entry so the next read re-resolves.
                self.cache.invalidate(&asset.id()).await?;
            }
        }

        self.assets.save(&asset).await?;
        for event in asset.drain_events() {
            self.publisher.publish(&event).await?;
        }
        Ok(ApplyModerationOutcome { applied: true, state: Some(asset.state()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::fakes::{t0, Fixture};
    use crate::domain::value_object::MediaKind;
    use uuid::Uuid;

    fn env(asset_id: AssetId, action: ModerationAction) -> Envelope<ApplyModerationCommand> {
        Envelope::new(Uuid::now_v7(), ApplyModerationCommand { asset_id, action })
    }

    #[tokio::test]
    async fn quarantine_revokes_delivery_then_restore_reinstates() {
        let fx = Fixture::new();
        let (asset_id, _owner) = fx.ready_asset(MediaKind::PostImage).await;
        fx.publisher.clear();

        let out = fx
            .apply_moderation_handler()
            .handle(env(asset_id, ModerationAction::Quarantine), t0())
            .await
            .unwrap();
        assert!(out.applied);
        assert_eq!(out.state, Some(AssetState::Quarantined));
        assert!(!fx.cdn.invalidated_keys().is_empty(), "delivery revoked at the edge");
        assert_eq!(fx.publisher.event_types(), vec!["media.asset_quarantined"]);

        fx.publisher.clear();
        let out = fx
            .apply_moderation_handler()
            .handle(env(asset_id, ModerationAction::Restore), t0())
            .await
            .unwrap();
        assert_eq!(out.state, Some(AssetState::Ready));
        assert_eq!(fx.publisher.event_types(), vec!["media.asset_restored"]);
    }

    #[tokio::test]
    async fn an_unknown_asset_is_a_folded_no_op() {
        let fx = Fixture::new();
        let out = fx
            .apply_moderation_handler()
            .handle(env(AssetId::new(), ModerationAction::Quarantine), t0())
            .await
            .unwrap();
        assert!(!out.applied);
        assert!(out.state.is_none());
    }
}
