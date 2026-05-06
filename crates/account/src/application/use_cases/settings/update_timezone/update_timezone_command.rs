use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, Timezone};
use shared_kernel::errors::{DomainError, Result};
use shared_proto::account::v1::UpdateTimezoneRequest;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTimezoneCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_timezone: Timezone,
}

impl UpdateTimezoneCommand {
    pub fn try_from_proto(req: UpdateTimezoneRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,

            new_timezone: Timezone::try_new(&req.timezone).map_err(|e| {
                DomainError::Validation {
                    field: "timezone",
                    reason: e.to_string(),
                }
            })?,
        })
    }
}
