// crates/profile/src/application/use_cases/create_profile/create_profile_command.rs

use crate::domain::value_objects::DisplayName;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProfileCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub display_name: DisplayName,
    pub username: Username,
}
