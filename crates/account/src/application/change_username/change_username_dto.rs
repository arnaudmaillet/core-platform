// crates/account/src/application/change_email/change_username_dto.rs

use crate::application::change_username::ChangeUsernameCommand;
use shared_kernel::domain::value_objects::Username;
use shared_kernel::errors::{DomainError, Result};

#[derive(serde::Deserialize)]
pub struct ChangeUsernameDto {
    pub account_id: String,
    pub new_username: String,
}

impl TryFrom<ChangeUsernameDto> for ChangeUsernameCommand {
    type Error = DomainError;
    fn try_from(dto: ChangeUsernameDto) -> Result<Self> {
        Ok(Self {
            account_id: dto.account_id.parse()?,
            new_username: Username::try_new(dto.new_username)?,
        })
    }
}
