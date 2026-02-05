// crates/account/src/application/change_email/change_phone_number_dto.rs

use crate::application::change_phone_number::change_phone_number_command::ChangePhoneNumberCommand;
use crate::domain::value_objects::PhoneNumber;
use shared_kernel::errors::{DomainError, Result};

#[derive(serde::Deserialize)]
pub struct ChangePhoneNumberDto {
    pub account_id: String,
    pub region_code: String,
    pub new_phone: String,
}

impl TryFrom<ChangePhoneNumberDto> for ChangePhoneNumberCommand {
    type Error = DomainError;
    fn try_from(dto: ChangePhoneNumberDto) -> Result<Self> {
        Ok(Self {
            account_id: dto.account_id.parse()?,
            region_code: dto.region_code.try_into()?,
            new_phone: PhoneNumber::try_new(dto.new_phone)?,
        })
    }
}
