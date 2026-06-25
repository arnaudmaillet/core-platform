use async_trait::async_trait;

use crate::domain::aggregate::Session;
use crate::domain::value_object::{AccountId, SessionId};
use crate::error::AuthError;

/// Durable persistence port for the [`Session`] aggregate (Postgres adapter,
/// sharded by `account_id`).
///
/// `save` upserts with optimistic-lock semantics on the aggregate `version`.
#[async_trait]
pub trait SessionRepository: Send + Sync + 'static {
    async fn save(&self, session: &Session) -> Result<(), AuthError>;

    async fn find_by_id(&self, id: &SessionId) -> Result<Option<Session>, AuthError>;

    /// Active sessions for an account — the device-management view and the set a
    /// global sign-out iterates over.
    async fn list_active_by_account(
        &self,
        account_id: &AccountId,
    ) -> Result<Vec<Session>, AuthError>;
}
