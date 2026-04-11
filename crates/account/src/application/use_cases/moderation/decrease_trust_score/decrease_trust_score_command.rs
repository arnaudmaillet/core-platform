// crates/account/src/application/decrease_trust_score/decrease_trust_score_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct DecreaseTrustScoreCommand {
    pub account_id: AccountId,
    pub action_id: Uuid,
    pub amount: u32,
    pub reason: String,
}
