// crates/account/src/application/decrease_trust_score/decrease_trust_score_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use uuid::Uuid;

use crate::domain::value_objects::TrustDelta;

#[derive(Debug, Deserialize, Clone)]
pub struct DecreaseTrustScoreCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub amount: TrustDelta,
    pub reason: AuditReason,
}
