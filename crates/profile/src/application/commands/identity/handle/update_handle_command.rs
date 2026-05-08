// crates/profile/src/application/commands/identity/handle/update_handle_command.rs

use crate::domain::value_objects::{Handle, ProfileId};
use serde::Deserialize;
use shared_kernel::domain::value_objects::RegionCode;
use shared_kernel::errors::{DomainError, Result};
use shared_proto::profile::v1::UpdateHandleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateHandleCommand {
    pub command_id: Uuid,
    pub profile_id: ProfileId,
    pub region: RegionCode,
    pub new_handle: Handle,
    pub expected_version: u64,
}

impl UpdateHandleCommand {
    pub fn try_from_proto(req: UpdateHandleRequest) -> Result<Self> {
        let metadata = req.metadata.ok_or_else(|| DomainError::Validation {
            field: "metadata",
            reason: "Missing command metadata".to_string(),
        })?;

        let target = req.target.ok_or_else(|| DomainError::Validation {
            field: "target",
            reason: "Missing profile target".to_string(),
        })?;

        Ok(Self {
            command_id: Uuid::parse_str(&metadata.command_id).map_err(|_| {
                DomainError::Validation {
                    field: "command_id",
                    reason: "Invalid UUID format".to_string(),
                }
            })?,
            profile_id: ProfileId::try_from(target.profile_id)?,
            region: RegionCode::from_str(&metadata.region)?,
            new_handle: Handle::from_raw(req.new_handle),
            expected_version: target.expected_version,
        })
    }
}
