// crates/account/src/application/register_account/register_account_command

use crate::domain::value_objects::{Email, ExternalId, IpAddr, Locale};
use shared_kernel::domain::value_objects::RegionCode;
use shared_proto::account::v1::RegisterRequest;

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub external_id: ExternalId,
    pub email: Email,
    pub region: RegionCode,
    pub locale: Locale,
    pub ip_addr: IpAddr,
}

impl RegisterCommand {
    pub fn try_from_proto(req: RegisterRequest, region: RegionCode) -> Result<Self, tonic::Status> {
        Ok(Self {
            external_id: ExternalId::from_raw(req.external_id),
            email: Email::try_new(req.email).map_err(|e| tonic::Status::invalid_argument(format!("Invalid email: {}", e)))?,
            region,
            locale: Locale::try_new(req.locale).map_err(|e| tonic::Status::invalid_argument(format!("Invalid locale: {}", e)))?,
            ip_addr: IpAddr::try_new(req.ip_addr).map_err(|e| tonic::Status::invalid_argument(format!("Invalid IP address: {}", e)))?,
        })
    }
}