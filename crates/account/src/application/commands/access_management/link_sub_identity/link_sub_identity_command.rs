// crates/account/src/application/link_sub_identity/link_sub_identity_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, RegionCode, SubId},
};
use shared_proto::account::v1::LinkSubIdentityRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LinkSubIdentityCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub sub_id: SubId,
}

impl IdentifiableCommand for LinkSubIdentityCommand {
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

impl LinkSubIdentityCommand {
    pub fn try_from_proto(req: LinkSubIdentityRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing account target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let sub_id =
            SubId::try_from(req.sub_id).map_err(|e| Error::validation("sub_id", e.to_string()))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        Ok(Self {
            command_id,
            target,
            sub_id,
        })
    }
}
