// crates/account/src/application/suspend_account/suspend_account_command
use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, AuditReason, Region},
};
use shared_proto::account::v1::SuspendRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct SuspendCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub reason: AuditReason,
}

impl IdentifiableCommand for SuspendCommand {
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

impl SuspendCommand {
    pub fn try_from_proto(req: SuspendRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("account_id", e.to_string()))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        Ok(Self {
            command_id,
            target,
            reason,
        })
    }
}
