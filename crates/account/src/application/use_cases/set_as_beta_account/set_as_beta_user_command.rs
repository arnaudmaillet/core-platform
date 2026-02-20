// crates/account/src/application/set_as_beta_account/set_as_beta_account_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct SetAsBetaAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub status: bool,
    pub reason: String,
}
