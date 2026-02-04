// crates/account/src/application/shadowban_account/shadowban_command.rs

use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ShadowbanAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub reason: String,
}
