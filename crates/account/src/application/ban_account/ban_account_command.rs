// crates/account/src/application/ban_account/ban_account_command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct BanAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub reason: String,
}