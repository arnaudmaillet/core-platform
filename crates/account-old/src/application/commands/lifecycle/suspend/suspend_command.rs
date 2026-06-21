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
    pub region: Region,
    pub reason: AuditReason,
}

impl IdentifiableCommand for SuspendCommand {
    type Id = AccountId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<AccountId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }
}

impl SuspendCommand {
    pub fn try_from_proto(req: SuspendRequest, region: Region) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("account_id", e.to_string()))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self {
            command_id,
            target,
            region,
            reason,
        })
    }
}
