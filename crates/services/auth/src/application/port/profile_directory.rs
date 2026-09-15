use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::value_object::{AccountId, ProfileId};
use crate::error::AuthError;

/// Outbound port to the `profile` service: which profiles does an account own?
///
/// Read at every mint (login and refresh) so the edge token's `pids` claim tracks
/// profile creation within one access-token lifetime. Auth never writes profiles.
#[async_trait]
pub trait ProfileDirectory: Send + Sync + 'static {
    /// The ids of every profile owned by `account_id`. Fails with
    /// [`AuthError::ProfileDirectoryUnavailable`] if the service is unreachable.
    async fn list_profile_ids(&self, account_id: &AccountId) -> Result<Vec<ProfileId>, AuthError>;
}

/// Resolves the `pids` claim **fail-safe**: a profile-service outage must not
/// block login (TIER-0 availability), and an empty claim only *removes* grants —
/// the caller can act as no profile until the next refresh succeeds. Logged at
/// warn so the degradation is visible.
pub async fn profile_ids_or_empty(
    directory: &Arc<dyn ProfileDirectory>,
    account_id: &AccountId,
) -> Vec<ProfileId> {
    match directory.list_profile_ids(account_id).await {
        Ok(ids) => ids,
        Err(error) => {
            tracing::warn!(
                account.id = %account_id,
                %error,
                "profile directory unavailable; minting an edge token with no profile grants"
            );
            Vec::new()
        }
    }
}
