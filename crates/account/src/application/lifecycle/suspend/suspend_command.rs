// crates/account/src/application/suspend_account/suspend_account_command
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct SuspendCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub reason: String,
}
