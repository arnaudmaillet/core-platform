// crates/profile/src/application/commands/identity/create_profile.rs

use crate::types::Handle;
use serde::{Deserialize, Serialize};
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, ProfileId, Region},
};
use shared_proto::profile::v1::CreateProfileRequest;
use uuid::Uuid;

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
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let account_id = AccountId::try_from(req.account_id.as_str())?;
        let handle = Handle::try_new(&req.handle)?;

        // On extrait la région passée par le client/la gateway à la création
        let region = Region::try_from(req.region.as_str())?;
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
