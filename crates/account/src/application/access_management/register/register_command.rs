// crates/account/src/application/register_account/register_account_command

use crate::domain::value_objects::{Email, ExternalId, IpAddr, Locale};
use shared_kernel::domain::value_objects::RegionCode;

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub external_id: ExternalId,
    pub email: Email,
    pub region: RegionCode,
    pub locale: Locale,
    pub ip_addr: IpAddr,
}
