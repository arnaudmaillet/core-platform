// crates/profile/src/application/use_cases/create_profile/create_profile_command.rs

use crate::domain::value_objects::{DisplayName, Handle};
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProfileCommand {
    pub owner_id: AccountId,
    pub region: RegionCode,
    pub display_name: DisplayName,
    pub handle: Handle,
}
