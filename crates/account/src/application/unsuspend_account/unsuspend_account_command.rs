// crates/account/src/application/unsuspend_account/unsuspend_account_command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct UnsuspendAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}
