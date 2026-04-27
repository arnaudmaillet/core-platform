// crates/account/src/application/update_locale/update_locale_command.rs

use crate::domain::value_objects::Locale;
use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateLocaleCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_locale: Locale,
}
