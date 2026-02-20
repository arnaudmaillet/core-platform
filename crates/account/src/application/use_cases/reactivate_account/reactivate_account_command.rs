// crates/account/src/application/reactivate_account/command.rs
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct ReactivateAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}
