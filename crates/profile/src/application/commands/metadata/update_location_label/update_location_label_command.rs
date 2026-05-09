// crates/profile/src/application/commands/metadata/update_location_label/update_location_label_command.rs

use crate::domain::value_objects::{Bio, ProfileId};
use serde::Deserialize;
use shared_kernel::application::{CommandTarget, IdentifiableCommand};
use shared_kernel::domain::value_objects::{LocationLabel, RegionCode};
use shared_kernel::errors::{DomainError, Result};
use shared_proto::profile::v1::UpdateLocationLabelRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateLocationLabelCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub new_location_label: Option<LocationLabel>,
}

impl IdentifiableCommand for UpdateLocationLabelCommand {
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

impl UpdateLocationLabelCommand {
    pub fn try_from_proto(req: UpdateLocationLabelRequest) -> Result<Self> {
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

        let new_location_label = req
            .new_location_label
            .filter(|s| !s.trim().is_empty())
            .map(|s| LocationLabel::try_new(s))
            .transpose()?;

        Ok(Self {
            command_id,
            target,
            new_location_label,
        })
    }
}
