// crates/account/src/application/link_sub_identity/link_sub_identity_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Region, SubId},
};
use shared_proto::account::v1::LinkSubIdentityRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct LinkSubIdentityCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub sub_id: SubId,
}

impl IdentifiableCommand for LinkSubIdentityCommand {
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

impl LinkSubIdentityCommand {
    pub fn try_from_proto(req: LinkSubIdentityRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing account target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let sub_id =
            SubId::try_from(req.sub_id).map_err(|e| Error::validation("sub_id", e.to_string()))?;

        let region = Region::try_new(proto_target.region)?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self {
            command_id,
            target,
            region,
            sub_id,
        })
    }
}
