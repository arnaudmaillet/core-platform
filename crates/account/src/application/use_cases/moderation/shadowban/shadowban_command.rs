// crates/account/src/application/shadowban_account/shadowban_command.rs

use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ShadowbanCommand {
    pub account_id: AccountId,
    pub reason: String,
}
