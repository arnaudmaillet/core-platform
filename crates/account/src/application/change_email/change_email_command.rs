// crates/account/src/application/change_email/change_email_command.rs

use shared_kernel::domain::value_objects::AccountId;
use crate::domain::value_objects::Email;

#[derive(Clone)]
pub struct ChangeEmailCommand {
    pub account_id: AccountId,
    pub new_email: Email,
}