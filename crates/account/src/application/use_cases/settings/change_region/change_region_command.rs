// crates/account/src/application/change_region/change_region_command.rs

use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::{AccountId, RegionCode},
    errors::{DomainError, Result},
};
use shared_proto::account::v1::ChangeRegionRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRegionCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_region: RegionCode,
}

impl ChangeRegionCommand {
    pub fn try_from_proto(req: ChangeRegionRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: AccountId::try_new(&req.account_id).map_err(|e| {
                DomainError::Validation {
                    field: "account_id",
                    reason: e.to_string(),
                }
            })?,

            new_region: RegionCode::try_new(&req.new_region).map_err(|e| {
                DomainError::Validation {
                    field: "new_region",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
