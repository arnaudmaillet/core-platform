//! One-time author-tier backfill.
//!
//! P1 emits `AuthorTierChanged` only when a follow/unfollow *crosses* a tier
//! boundary — so an author whose follower count already sits above a threshold
//! (i.e. predates the feature) never emits, and the denormalization chain
//! (profile → post → timeline) would treat them as Standard forever. This backfill
//! recomputes each author's tier from its current follower count and emits the
//! non-Standard ones, seeding the fleet so existing celebrities are classified on
//! day one rather than being write-fanned-out.
//!
//! Triggering is an operator concern: social-graph has no all-authors index, so
//! the driver supplies the `profile_id` set (e.g. a `profile` export, or a scan of
//! the `followers` partitions) and invokes [`backfill_tiers`].

use chrono::Utc;

use crate::application::port::{EventPublisher, SocialGraphCache};
use crate::domain::event::{AuthorTierChanged, DomainEvent};
use crate::domain::value_object::{AuthorTier, ProfileId, TierThresholds};
use crate::error::SocialGraphError;

/// Recompute one author's tier from its current follower count and emit
/// `AuthorTierChanged` when it is non-Standard. Returns the computed tier.
///
/// Standard (the default every consumer assumes) is intentionally **not** emitted,
/// keeping the backfill to the authors that actually need a tier.
pub async fn recompute_and_emit_tier(
    cache: &dyn SocialGraphCache,
    publisher: &dyn EventPublisher,
    thresholds: TierThresholds,
    profile_id: &ProfileId,
) -> Result<AuthorTier, SocialGraphError> {
    let counts = cache.get_counts(profile_id).await?;
    let tier = AuthorTier::from_follower_count(counts.followers, thresholds);

    if tier != AuthorTier::Standard {
        publisher
            .publish(&DomainEvent::AuthorTierChanged(AuthorTierChanged {
                profile_id: *profile_id,
                new_tier: tier,
                follower_count: counts.followers,
                changed_at: Utc::now(),
            }))
            .await?;
    }
    Ok(tier)
}

/// Backfill a batch of authors. A failure on one author is logged and skipped so a
/// single bad profile never aborts the run. Returns the number of non-Standard
/// tiers emitted.
pub async fn backfill_tiers(
    cache: &dyn SocialGraphCache,
    publisher: &dyn EventPublisher,
    thresholds: TierThresholds,
    profile_ids: &[ProfileId],
) -> usize {
    let mut emitted = 0;
    for profile_id in profile_ids {
        match recompute_and_emit_tier(cache, publisher, thresholds, profile_id).await {
            Ok(tier) if tier != AuthorTier::Standard => emitted += 1,
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(%error, %profile_id, "tier backfill failed for profile; skipping")
            }
        }
    }
    emitted
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;

    use super::*;
    use crate::application::port::RelationCounts;

    /// A cache that reports a fixed follower count and stubs the rest.
    struct FixedCountCache {
        followers: i64,
    }

    #[async_trait]
    impl SocialGraphCache for FixedCountCache {
        async fn add_following(&self, _: &ProfileId, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn remove_following(&self, _: &ProfileId, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn add_block(&self, _: &ProfileId, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn remove_block(&self, _: &ProfileId, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn incr_followers_count(&self, _: &ProfileId) -> Result<i64, SocialGraphError> {
            Ok(self.followers)
        }
        async fn decr_followers_count(&self, _: &ProfileId) -> Result<i64, SocialGraphError> {
            Ok(self.followers)
        }
        async fn incr_following_count(&self, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn decr_following_count(&self, _: &ProfileId) -> Result<(), SocialGraphError> {
            Ok(())
        }
        async fn get_counts(&self, _: &ProfileId) -> Result<RelationCounts, SocialGraphError> {
            Ok(RelationCounts { followers: self.followers, following: 0 })
        }
    }

    #[derive(Default)]
    struct CapturingPublisher {
        events: Mutex<Vec<DomainEvent>>,
    }

    #[async_trait]
    impl EventPublisher for CapturingPublisher {
        async fn publish(&self, event: &DomainEvent) -> Result<(), SocialGraphError> {
            self.events.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    fn thresholds() -> TierThresholds {
        TierThresholds::new(10_000, 1_000_000)
    }

    fn author() -> ProfileId {
        ProfileId::try_from(uuid::Uuid::now_v7().to_string().as_str()).unwrap()
    }

    #[tokio::test]
    async fn emits_for_a_vip_author() {
        let cache = FixedCountCache { followers: 2_000_000 };
        let publisher = CapturingPublisher::default();

        let tier = recompute_and_emit_tier(&cache, &publisher, thresholds(), &author())
            .await
            .unwrap();

        assert_eq!(tier, AuthorTier::Vip);
        let events = publisher.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DomainEvent::AuthorTierChanged(_)));
    }

    #[tokio::test]
    async fn does_not_emit_for_a_standard_author() {
        let cache = FixedCountCache { followers: 42 };
        let publisher = CapturingPublisher::default();

        let tier = recompute_and_emit_tier(&cache, &publisher, thresholds(), &author())
            .await
            .unwrap();

        assert_eq!(tier, AuthorTier::Standard);
        assert!(publisher.events.lock().unwrap().is_empty());
    }
}
