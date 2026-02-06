// crates/account/src/application/add_push_token/add_push_token_command.rs

use shared_kernel::domain::value_objects::{AccountId, PushToken, RegionCode};

#[derive(Debug, Clone)]
pub struct AddPushTokenCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub token: PushToken,
}
