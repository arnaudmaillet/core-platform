// crates/account/src/application/add_push_token/add_push_token_command.rs

use shared_kernel::domain::value_objects::{AccountId, PushToken};

#[derive(Debug, Clone)]
pub struct AddPushTokenCommand {
    pub account_id: AccountId,
    pub token: PushToken,
}
