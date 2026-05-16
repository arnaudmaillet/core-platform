// crates/account/src/application/increase_trust_score/command.rs

use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, AuditReason, RegionCode},
};
use shared_proto::account::v1::IncreaseTrustScoreRequest;
use uuid::Uuid;

use crate::types::TrustAmount;

#[derive(Debug, Deserialize, Clone)]
pub struct IncreaseTrustScoreCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub amount: TrustAmount,
    pub reason: AuditReason,
}

impl IdentifiableCommand for IncreaseTrustScoreCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl IncreaseTrustScoreCommand {
    pub fn try_from_proto(req: IncreaseTrustScoreRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };
        let amount = TrustAmount::try_from(req.amount)
            .map_err(|e| Error::validation("amount", e.to_string()))?;

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("reason", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            reason,
            amount,
        })
    }
}
