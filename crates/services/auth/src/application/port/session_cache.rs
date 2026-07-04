use async_trait::async_trait;
use chrono::Duration;

use crate::domain::value_object::{AccountId, Generation, SessionId};
use crate::error::AuthError;

/// Hot-path cache port (Redis Cluster adapter, hash-tag slot-safe).
///
/// This is what keeps authentication off the durable store on the request path:
/// the per-account generation and the per-session blacklist are O(1) reads. The
/// durable Postgres row remains the source of truth; the cache is a write-through
/// projection that a miss can rebuild.
#[async_trait]
pub trait SessionCache: Send + Sync + 'static {
    /// The account's current revocation epoch. A miss resolves to
    /// [`Generation::INITIAL`] and should be re-warmed from the durable store by
    /// the adapter.
    async fn current_generation(&self, account_id: &AccountId) -> Result<Generation, AuthError>;

    /// Atomically bumps and returns the account's generation — the global
    /// sign-out kill switch.
    async fn bump_generation(&self, account_id: &AccountId) -> Result<Generation, AuthError>;

    /// Marks a single session revoked for `ttl` (≥ the max access-token lifetime),
    /// so an already-minted edge token is rejected before its natural expiry.
    async fn blacklist_session(
        &self,
        session_id: &SessionId,
        ttl: Duration,
    ) -> Result<(), AuthError>;

    async fn is_blacklisted(&self, session_id: &SessionId) -> Result<bool, AuthError>;
}
