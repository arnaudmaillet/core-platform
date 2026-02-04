use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveBannerCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
}
