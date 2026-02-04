// crates/account/src/application/change_email/change_email_command.rs

use crate::domain::value_objects::Email;
use shared_kernel::domain::value_objects::AccountId;

#[derive(Clone)]
pub struct ChangeEmailCommand {
    pub account_id: AccountId,
    pub new_email: Email,
}
