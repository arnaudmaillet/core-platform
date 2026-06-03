// crates/account/src/application/change_role/change_role_command.rs

use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Region},
};
use shared_proto::account::v1::ChangeBetaTierRequest;
use uuid::Uuid;

use crate::types::BetaTier;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBetaTierCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_tier: BetaTier,
}

impl IdentifiableCommand for ChangeBetaTierCommand {
    type Id = AccountId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<AccountId> {
        &self.target
    }
}

impl ChangeBetaTierCommand {
    pub fn try_from_proto(req: ChangeBetaTierRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let new_tier = BetaTier::try_from(req.new_tier)
            .map_err(|e| Error::validation("new_tier", e.to_string()))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self {
            command_id,
            target,
            new_tier,
        })
    }
}
