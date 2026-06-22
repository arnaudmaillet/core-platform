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
