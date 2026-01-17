// crates/account/src/application/remove_push_token/remove_push_token_command.rs

use shared_kernel::domain::value_objects::{PushToken, AccountId};

#[derive(Debug, Clone)]
pub struct RemovePushTokenCommand {
    pub account_id: AccountId,
    pub token: PushToken,
}