// crates/account/src/application/change_email/change_phone_number_command.rs

use shared_kernel::domain::value_objects::AccountId;
use crate::domain::value_objects::PhoneNumber;

#[derive(Clone)]
pub struct ChangePhoneNumberCommand {
    pub account_id: AccountId,
    pub new_phone: PhoneNumber,
}