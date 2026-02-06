// crates/account/src/application/link_external_identity/link_external_identity_command.rs

use crate::domain::value_objects::ExternalId;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone)]
pub struct LinkExternalIdentityCommand {
    pub internal_account_id: AccountId,
    pub region_code: RegionCode,
    pub external_id: ExternalId,
}
