// crates/account/src/application/change_region/change_region_command.rs

use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Region},
};
use shared_proto::account::v1::ChangeRegionRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRegionCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
pub region: Region,
    pub new_region: Region,
}

impl IdentifiableCommand for ChangeRegionCommand {
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

impl ChangeRegionCommand {
    pub fn try_from_proto(req: ChangeRegionRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        let new_region = Region::try_new(&req.new_region)
            .map_err(|e| Error::validation("new_region", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            new_region,
        })
    }
}
