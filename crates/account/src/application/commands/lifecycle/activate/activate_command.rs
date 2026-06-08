// crates/account/src/application/reactivate_account/command.rs
use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Region},
};
use shared_proto::account::v1::ActivateRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ActivateCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
}

impl IdentifiableCommand for ActivateCommand {
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

impl ActivateCommand {
    pub fn try_from_proto(req: ActivateRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        let region = Region::try_new(proto_target.region)?;

        Ok(Self {
            command_id,
            region,
            target,
        })
    }
}
