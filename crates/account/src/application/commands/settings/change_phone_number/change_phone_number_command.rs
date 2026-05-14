// crates/account/src/application/change_email/change_phone_number_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, PhoneNumber, RegionCode},
};
use shared_proto::account::v1::ChangePhoneNumberRequest;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChangePhoneNumberCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_phone: PhoneNumber,
}

impl IdentifiableCommand for ChangePhoneNumberCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }
}

impl ChangePhoneNumberCommand {
    pub fn try_from_proto(req: ChangePhoneNumberRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: RegionCode::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        let new_phone = PhoneNumber::try_from(req.new_phone)
            .map_err(|e| Error::validation("new_phone", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            new_phone,
        })
    }
}
