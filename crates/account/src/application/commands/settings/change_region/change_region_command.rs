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
    pub new_region: Region,
}

impl IdentifiableCommand for ChangeRegionCommand {
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

impl ChangeRegionCommand {
    pub fn try_from_proto(req: ChangeRegionRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
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
