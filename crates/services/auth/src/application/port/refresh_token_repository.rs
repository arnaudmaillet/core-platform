use async_trait::async_trait;

use crate::domain::aggregate::RefreshToken;
use crate::domain::value_object::{RefreshTokenHash, SessionId};
use crate::error::AuthError;

/// Durable persistence port for refresh tokens (Postgres adapter, sharded by
/// `account_id`).
///
/// Lookup is by hash: the presented plaintext is hashed and matched against the
/// stored value, so the plaintext never has to be retained.
#[async_trait]
pub trait RefreshTokenRepository: Send + Sync + 'static {
    /// Upserts a token row (insert on issue, update on rotate/revoke).
    async fn save(&self, token: &RefreshToken) -> Result<(), AuthError>;

    /// Finds a token by its stored hash, or `None` if absent.
    async fn find_by_hash(
        &self,
        hash: &RefreshTokenHash,
    ) -> Result<Option<RefreshToken>, AuthError>;

    /// Invalidates every outstanding token for a session (logout / reuse-defence).
    async fn revoke_all_for_session(&self, session_id: &SessionId) -> Result<(), AuthError>;
}
