// crates/account/src/application/register_account/register_account_command

use crate::domain::value_objects::{
    Email, SubId, IpAddr, Locale, PhoneNumber, RegistrationIdentifier,
};
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_proto::account::v1::{RegisterRequest, registration_identifier::Method};
use tonic::Status;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub sub_id: Option<SubId>,
    pub identifier: RegistrationIdentifier,
    pub region: RegionCode,
    pub locale: Locale,
    pub ip_addr: IpAddr,
}

impl RegisterCommand {
    pub fn try_from_proto(req: RegisterRequest, account_id: AccountId) -> Result<Self, tonic::Status> {
        let identifier = match req.identifier.and_then(|i| i.method) {
            Some(Method::Email(e)) => RegistrationIdentifier::from_email(
                Email::try_new(e).map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            Some(Method::PhoneNumber(p)) => RegistrationIdentifier::from_phone(
                PhoneNumber::try_new(p).map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            Some(Method::Both(b)) => RegistrationIdentifier::from_both(
                Email::try_new(b.email).map_err(|err| Status::invalid_argument(err.to_string()))?,
                PhoneNumber::try_new(b.phone_number)
                    .map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            None => return Err(Status::invalid_argument("Missing registration identifier")),
        };

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid CommandId: {}", e)))?,
            account_id: account_id,
            sub_id: match req.sub_id {
                Some(id) if !id.is_empty() => Some(
                    SubId::try_new(id).map_err(|e| Status::invalid_argument(e.to_string()))?,
                ),
                _ => None,
            },
            identifier,
            region: RegionCode::try_new(req.region_code)
                .map_err(|e| Status::invalid_argument(format!("Invalid region: {}", e)))?,
            locale: Locale::try_new(req.locale)
                .map_err(|e| Status::invalid_argument(format!("Invalid locale: {}", e)))?,
            ip_addr: IpAddr::try_new(req.ip_addr)
                .map_err(|e| Status::invalid_argument(format!("Invalid IP address: {}", e)))?,
        })
    }
}
