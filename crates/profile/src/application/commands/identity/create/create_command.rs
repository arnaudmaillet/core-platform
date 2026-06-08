// crates/profile/src/application/commands/identity/create_profile.rs

use serde::{Deserialize, Serialize};
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, ProfileId, Region},
};
use shared_proto::profile::v1::CreateProfileRequest;
use uuid::Uuid;

use crate::types::Handle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProfileCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
    pub account_id: AccountId,
    pub handle: Handle,
}

impl IdentifiableCommand for CreateProfileCommand {
    type Id = ProfileId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }
}

impl CreateProfileCommand {
    pub fn try_from_proto(req: CreateProfileRequest, profile_id: ProfileId) -> Result<Self> {
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format".to_string()))?;

        let account_id = AccountId::try_from(req.account_id.as_str())?;
        let handle = Handle::try_new(&req.handle)?;
        let region = Region::try_new(&req.region)?;
        let target = CommandTarget::stateless(profile_id);

        Ok(Self {
            command_id,
            target,
            region,
            account_id,
            handle,
        })
    }
}
