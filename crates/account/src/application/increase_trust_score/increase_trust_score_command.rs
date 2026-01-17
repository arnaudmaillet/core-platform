// crates/account/src/application/increase_trust_score/command.rs

use serde::Deserialize;
use uuid::Uuid;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct IncreaseTrustScoreCommand {
    pub action_id: Uuid,
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub amount: u32,
    pub reason: String,
}