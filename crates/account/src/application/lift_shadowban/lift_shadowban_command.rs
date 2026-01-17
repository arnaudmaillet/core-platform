// crates/account/src/application/lift_shadowban/lift_shadowban_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct LiftShadowbanCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub reason: String,
}