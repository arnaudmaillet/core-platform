// crates/account/src/application/change_birth_date/command.rs

use crate::domain::value_objects::BirthDate;
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBirthDateCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub birth_date: BirthDate,
}
