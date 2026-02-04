// crates/account/src/application/change_email/change_accountname_command.rs

use shared_kernel::domain::value_objects::{AccountId, Username};

#[derive(Clone)]
pub struct ChangeUsernameCommand {
    pub account_id: AccountId,
    pub new_username: Username,
}
