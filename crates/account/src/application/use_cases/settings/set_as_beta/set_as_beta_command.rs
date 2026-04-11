// crates/account/src/application/set_as_beta_account/set_as_beta_account_command.rs

use serde::Deserialize;
use shared_kernel::domain::value_objects::AccountId;

#[derive(Debug, Deserialize, Clone)]
pub struct SetAsBetaCommand {
    pub account_id: AccountId,
    pub status: bool,
    pub reason: String,
}
