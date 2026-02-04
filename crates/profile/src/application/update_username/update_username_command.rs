// crates/profile/src/application/use_cases/update_username/update_username_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUsernameCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_username: Username,
}
