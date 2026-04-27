// crates/account/src/application/remove_push_token/remove_push_token_command.rs

use shared_kernel::domain::value_objects::{AccountId, PushToken};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RemovePushTokenCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub token: PushToken,
}
