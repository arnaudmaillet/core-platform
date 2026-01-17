// crates/profile/src/application/use_cases/update_privacy/update_privacy_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{RegionCode, AccountId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePrivacyCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub is_private: bool,
}