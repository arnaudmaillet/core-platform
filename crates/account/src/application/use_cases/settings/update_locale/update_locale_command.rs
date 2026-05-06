// crates/account/src/application/update_locale/update_locale_command.rs

use crate::domain::value_objects::Locale;
use serde::Deserialize;
use shared_kernel::{
    domain::value_objects::AccountId,
    errors::{DomainError, Result},
};
use shared_proto::account::v1::UpdateLocaleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateLocaleCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_locale: Locale,
}

impl UpdateLocaleCommand {
    pub fn try_from_proto(req: UpdateLocaleRequest) -> Result<Self> {
        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id).map_err(|_| DomainError::Validation {
                field: "command_id",
                reason: "Invalid UUID format".to_string(),
            })?,

            account_id: req.account_id.parse().map_err(|e: DomainError| e)?,

            new_locale: Locale::try_new(&req.locale).map_err(|e| DomainError::Validation {
                field: "new_locale",
                reason: e.to_string(),
            })?,
        })
    }
}
