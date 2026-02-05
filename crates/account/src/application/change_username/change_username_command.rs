// crates/account/src/application/change_username/change_username_command.rs

use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};

#[derive(Clone)]
pub struct ChangeUsernameCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub new_username: Username,
}
