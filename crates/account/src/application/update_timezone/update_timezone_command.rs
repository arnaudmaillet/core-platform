use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::domain::value_objects::Timezone;

#[derive(Debug, Deserialize)]
pub struct UpdateTimezoneCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub new_timezone: Timezone,
}
