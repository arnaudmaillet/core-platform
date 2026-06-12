use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{AccountId, AuditReason, Region};
use shared_proto::account::v1::DeactivateRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub reason: Option<AuditReason>,
}

impl IdentifiableCommand for DeactivateCommand {
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

impl DeactivateCommand {
    pub fn try_from_proto(req: DeactivateRequest, region: Region) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let reason = req
            .reason
            .map(AuditReason::try_new)
            .transpose()
            .map_err(|e| Error::validation("reason", e.to_string()))?;

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
