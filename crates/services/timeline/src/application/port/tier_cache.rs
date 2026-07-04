use async_trait::async_trait;

use crate::domain::value_object::{AuthorId, AuthorTier, ProfileId};
use crate::error::TimelineError;

/// Port for short-lived author tier lookups and feed warm-state flags.
///
/// Two concerns are co-located here because both use the same Redis client
/// and both are single-key STRING operations with TTL semantics.
///
/// Key patterns:
///   `timeline:tier:{author_id}`  — AuthorTier serialized as "0", "1", or "2"
///   `timeline:warm:{profile_id}` — existence flag; "1" EX warm_ttl_secs
#[async_trait]
pub trait TierCache: Send + Sync + 'static {
    /// Returns the cached tier for an author, or `None` on cache miss.
    ///
    /// The caller is responsible for resolving cache misses from an external
    /// source (e.g., a tier embedded in the Kafka event) and calling `set_tier`.
    async fn get_tier(&self, author_id: &AuthorId) -> Result<Option<AuthorTier>, TimelineError>;

    /// Batch variant of [`get_tier`], returning tiers in the same order as
    /// `author_ids` (`None` per cache miss).
    ///
    /// The default implementation issues sequential lookups — correct but one
    /// round-trip per author. Production adapters must override it with a
    /// pipelined/concurrent batch; the read path calls this once per feed request
    /// over the caller's entire following list, so a serial default would be a
    /// latency cliff for users following thousands of accounts.
    async fn get_tiers(
        &self,
        author_ids: &[AuthorId],
    ) -> Result<Vec<Option<AuthorTier>>, TimelineError> {
        let mut tiers = Vec::with_capacity(author_ids.len());
        for author_id in author_ids {
            tiers.push(self.get_tier(author_id).await?);
        }
        Ok(tiers)
    }

    /// Caches the author's tier with a TTL of `ttl_secs`.
    async fn set_tier(
        &self,
        author_id: &AuthorId,
        tier:      AuthorTier,
        ttl_secs:  u64,
    ) -> Result<(), TimelineError>;

    /// Returns true if the `timeline:warm:{profile_id}` flag is set.
    async fn is_warm(&self, profile_id: &ProfileId) -> Result<bool, TimelineError>;

    /// Sets the warm flag for a profile, signalling that the Redis feed is
    /// populated and no cold-start rebuild is needed for `ttl_secs` seconds.
    async fn set_warm(&self, profile_id: &ProfileId, ttl_secs: u64) -> Result<(), TimelineError>;
}
