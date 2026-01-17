// crates/account/src/application/unban_account/unban_command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct UnbanAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}