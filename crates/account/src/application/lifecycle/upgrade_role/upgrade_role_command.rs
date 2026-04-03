// crates/account/src/application/upgrade_role/upgrade_role_command.rs

use crate::domain::value_objects::AccountRole;
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct UpgradeRoleCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub new_role: AccountRole,
    pub reason: String,
}
