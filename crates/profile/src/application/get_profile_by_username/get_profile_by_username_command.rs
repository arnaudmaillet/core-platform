// crates/profile/src/application/queries/get_profile_by_username/get_profile_by_username_command

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{RegionCode, Username};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProfileByUsernameCommand {
    pub username: Username,
    pub region: RegionCode,
}