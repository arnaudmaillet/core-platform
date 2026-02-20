use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct DeactivateAccountCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
}
