use shared_kernel::domain::value_objects::{AccountId, RegionCode};

pub struct VerifyEmailCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub token: String,
}
