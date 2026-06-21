// crates/profile/src/application/commands/media/remove_banner/remove_banner_command.rs

use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::ProfileId;
use shared_proto::profile::v1::RemoveBannerRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct RemoveBannerCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
}

impl IdentifiableCommand for RemoveBannerCommand {
    type Id = ProfileId;
    type Routing = ();

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        ()
    }
}

impl RemoveBannerCommand {
    pub fn try_from_proto(req: RemoveBannerRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: ProfileId::try_new(proto_target.profile_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self { command_id, target })
    }
}
