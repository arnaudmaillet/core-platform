// crates/account/src/application/shadowban_account/shadowban_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, AuditReason, RegionCode},
};
use shared_proto::account::v1::ModerationRequest;
use uuid::Uuid;

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ShadowbanCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub reason: AuditReason,
}

impl IdentifiableCommand for ShadowbanCommand {
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

impl ShadowbanCommand {
    pub fn try_from_proto(req: ModerationRequest) -> Result<Self> {
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

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("reason", e.to_string()))?;
        Ok(Self {
            command_id,
            target,
            reason,
        })
    }
}
