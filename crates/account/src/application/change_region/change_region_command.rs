// crates/account/src/application/change_region/change_region_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRegionCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode, // La région actuelle (pour router la requête SQL)
    pub new_region: RegionCode,   // La destination cible
}