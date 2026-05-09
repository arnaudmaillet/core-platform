// crates/profile/src/application/commands/metadata/update_social_links/update_social_links_command.rs

use crate::commands::metadata::update_social_links::mapper::from_proto_to_social_links;
use crate::domain::value_objects::{ProfileId, SocialLinks};
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::{DomainError, Result};
use shared_proto::profile::v1::UpdateSocialLinksRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateSocialLinksCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_links: Option<SocialLinks>,
}

impl IdentifiableCommand for UpdateSocialLinksCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn profile_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl UpdateSocialLinksCommand {
    pub fn try_from_proto(req: UpdateSocialLinksRequest) -> Result<Self> {
        let proto_target = req.target.ok_or_else(|| DomainError::Validation {
            field: "target",
            reason: "Missing profile target".to_string(),
        })?;

        let command_id = Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
            field: "command_id",
            reason: "Invalid UUID format".to_string(),
        })?;

        let target = CommandTarget {
            id: ProfileId::try_new(proto_target.profile_id)?,
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        let new_links = req
            .new_links
            .map(from_proto_to_social_links)
            .transpose()?
            .flatten();

        Ok(Self {
            command_id,
            target,
            new_links,
        })
    }
}
