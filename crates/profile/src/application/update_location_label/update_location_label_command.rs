// crates/profile/src/application/use_cases/update_location/update_location_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{LocationLabel, RegionCode, AccountId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLocationLabelCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_location: Option<LocationLabel>,
}