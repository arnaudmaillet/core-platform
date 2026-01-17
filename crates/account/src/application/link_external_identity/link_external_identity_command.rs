// crates/account/src/application/link_external_identity/link_external_identity_command.rs

use shared_kernel::domain::value_objects::AccountId;
use crate::domain::value_objects::ExternalId;

#[derive(Debug, Clone)]
pub struct LinkExternalIdentityCommand {
    pub internal_account_id: AccountId,
    pub external_id: ExternalId,
}