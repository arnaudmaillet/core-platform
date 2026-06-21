use std::sync::Arc;

use uuid::Uuid;

use crate::application::port::AccountRepository;
use crate::domain::aggregate::Account;
use crate::domain::value_object::AccountId;
use crate::error::AccountError;

/// Loads an account by its string-encoded UUID, returning structured errors
/// for both invalid UUID format and missing records.
pub(crate) async fn load_account(
    repo: &Arc<dyn AccountRepository>,
    id_str: &str,
) -> Result<Account, AccountError> {
    let uuid = id_str.parse::<Uuid>().map_err(|_| AccountError::DomainViolation {
        field: "account_id".into(),
        message: "invalid UUID format".into(),
    })?;
    let id = AccountId::from_uuid(uuid);
    repo.find_by_id(&id)
        .await?
        .ok_or_else(|| AccountError::AccountNotFound { id: id_str.to_owned() })
}
