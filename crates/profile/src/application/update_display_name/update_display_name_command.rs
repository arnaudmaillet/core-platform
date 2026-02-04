// crates/profile/src/application/use_cases/update_display_name/update_display_name_command.rs

use crate::domain::value_objects::DisplayName;
use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDisplayNameCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_display_name: DisplayName,
}
