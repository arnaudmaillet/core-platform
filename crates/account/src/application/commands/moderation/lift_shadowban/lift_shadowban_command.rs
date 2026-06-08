// crates/account/src/application/lift_shadowban/lift_shadowban_command.rs

use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, AuditReason, Region},
};
use shared_proto::account::v1::LiftShadowbanRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct LiftShadowbanCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub reason: AuditReason,
}

impl IdentifiableCommand for LiftShadowbanCommand {
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

impl LiftShadowbanCommand {
    pub fn try_from_proto(req: LiftShadowbanRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("reason", e.to_string()))?;

        let region = Region::try_new(proto_target.region)?;

        Ok(Self {
            command_id,
            target,
            region,
            reason,
        })
    }
}
