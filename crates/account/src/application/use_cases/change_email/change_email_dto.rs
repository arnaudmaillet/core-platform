// crates/account/src/application/change_email/change_email_dto.rs

use crate::application::use_cases::change_email::ChangeEmailCommand;
use shared_kernel::errors::{DomainError, Result};

pub struct ChangeEmailDto {
    pub account_id: String,
    pub region_code: String,
    pub new_email: String,
}

impl TryFrom<ChangeEmailDto> for ChangeEmailCommand {
    type Error = DomainError;

    fn try_from(dto: ChangeEmailDto) -> Result<Self> {
        Ok(Self {
            account_id: dto.account_id.parse()?,
            region_code: dto.region_code.try_into()?,
            new_email: dto.new_email.try_into()?,
        })
    }
}
