// crates/profile/src/application/use_cases/update_bio/update_bio_command.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::{RegionCode, AccountId};
use crate::domain::value_objects::Bio;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBioCommand {
    pub account_id: AccountId,
    pub region: RegionCode,
    pub new_bio: Option<Bio>,
}