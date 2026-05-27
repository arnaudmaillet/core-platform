// crates/profile/src/application/commands/identity/create_profile.rs

use serde::{Deserialize, Serialize};
use shared_kernel::{
    command::IdentifiableCommand,
    core::{Error, Result},
    types::{AccountId, ProfileId, Region},
};
use shared_proto::profile::v1::CreateProfileRequest;
use uuid::Uuid;

use crate::types::Handle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProfileCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub profile_id: ProfileId,
    pub handle: Handle,
    pub region: Region,
}

impl IdentifiableCommand for CreateProfileCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }
    fn aggregate_id(&self) -> String {
        self.account_id.to_string()
    }
    fn region(&self) -> String {
        self.region.to_string()
    }
}

impl CreateProfileCommand {
    pub fn try_from_proto(req: CreateProfileRequest, profile_id: ProfileId) -> Result<Self> {
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format".to_string()))?;

        let account_id = AccountId::try_from(req.account_id.as_str())?;
        let handle = Handle::try_new(&req.handle)?;
        let region = Region::try_new(&req.region)?;

        Ok(Self {
            command_id,
            account_id,
            profile_id,
            handle,
            region,
        })
    }
}
