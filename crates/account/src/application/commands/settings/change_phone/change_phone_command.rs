// crates/account/src/application/change_email/change_phone_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Phone, Region},
};
use shared_proto::account::v1::ChangePhoneRequest;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChangePhoneCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_phone: Phone,
}

impl IdentifiableCommand for ChangePhoneCommand {
    type Id = AccountId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<AccountId> {
        &self.target
    }
}

impl ChangePhoneCommand {
    pub fn try_from_proto(req: ChangePhoneRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: Some(proto_target.expected_version),
        };

        let new_phone = Phone::try_from(req.new_phone)
            .map_err(|e| Error::validation("new_phone", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            new_phone,
        })
    }
}
