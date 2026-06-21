// crates/account/src/application/register/register_command.rs

use crate::domain::types::{IpAddr, Locale, RegistrationIdentifier};
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    types::{AccountId, Email, Phone, Region, SubId},
};
use shared_proto::account::v1::{RegisterRequest, registration_identifier::Method};
use tonic::Status;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RegisterCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub sub_id: Option<SubId>,
    pub identifier: RegistrationIdentifier,
    pub locale: Locale,
    pub ip_addr: IpAddr,
}

impl IdentifiableCommand for RegisterCommand {
    type Id = AccountId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<AccountId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }
}

impl RegisterCommand {
    pub fn try_from_proto(
        req: RegisterRequest,
        account_id: AccountId,
        region: Region,
    ) -> Result<Self, tonic::Status> {
        let identifier = match req.identifier.and_then(|i| i.method) {
            Some(Method::Email(e)) => RegistrationIdentifier::from_email(
                Email::try_new(e).map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            Some(Method::Phone(p)) => RegistrationIdentifier::from_phone(
                Phone::try_new(p).map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            Some(Method::Both(b)) => RegistrationIdentifier::from_both(
                Email::try_new(b.email).map_err(|err| Status::invalid_argument(err.to_string()))?,
                Phone::try_new(b.phone).map_err(|err| Status::invalid_argument(err.to_string()))?,
            ),
            None => return Err(Status::invalid_argument("Missing registration identifier")),
        };

        let target = CommandTarget::stateless(account_id);

        Ok(Self {
            command_id: Uuid::parse_str(&req.command_id)
                .map_err(|e| Status::invalid_argument(format!("Invalid CommandId: {}", e)))?,
            target,
            region,
            sub_id: match req.sub_id {
                Some(id) if !id.is_empty() => {
                    Some(SubId::try_new(id).map_err(|e| Status::invalid_argument(e.to_string()))?)
                }
                _ => None,
            },
            identifier,
            locale: Locale::try_new(req.locale)
                .map_err(|e| Status::invalid_argument(format!("Invalid locale: {}", e)))?,
            ip_addr: IpAddr::try_new(req.ip_addr)
                .map_err(|e| Status::invalid_argument(format!("Invalid IP address: {}", e)))?,
        })
    }
}
