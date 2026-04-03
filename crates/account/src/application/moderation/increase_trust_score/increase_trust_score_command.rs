// crates/account/src/application/increase_trust_score/command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct IncreaseTrustScoreCommand {
    pub action_id: Uuid,
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub amount: u32,
    pub reason: String,
}
