// crates/account/src/application/change_email/change_email_dto.rs

use crate::application::change_email::ChangeEmailCommand;
use shared_kernel::errors::{DomainError, Result};

pub struct ChangeEmailDto {
    pub account_id: String,
    pub new_email: String,
}

impl TryFrom<ChangeEmailDto> for ChangeEmailCommand {
    type Error = DomainError;

    fn try_from(dto: ChangeEmailDto) -> Result<Self> {
        Ok(Self {
            account_id: dto.account_id.parse()?,
            new_email: dto.new_email.try_into()?,
        })
    }
}
