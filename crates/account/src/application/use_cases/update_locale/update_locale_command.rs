// crates/account/src/application/update_locale/update_locale_command.rs

use crate::domain::value_objects::Locale;
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateLocaleCommand {
    pub account_id: AccountId,
    pub region_code: RegionCode,
    pub locale: Locale,
}
