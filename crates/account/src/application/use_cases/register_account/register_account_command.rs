// crates/account/src/application/register_account/register_account_command

use crate::domain::value_objects::{Email, ExternalId, Locale};
use shared_kernel::domain::value_objects::{RegionCode, Username};

#[derive(Debug, Clone)]
pub struct RegisterAccountCommand {
    pub external_id: ExternalId,
    pub username: Username,
    pub email: Email,
    pub region: RegionCode,
    pub locale: Locale,
    pub ip_address: Option<String>,
}
