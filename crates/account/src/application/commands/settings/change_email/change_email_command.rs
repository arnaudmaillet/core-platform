// crates/account/src/application/change_email/change_email_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, Email, Region},
};
use shared_proto::account::v1::ChangeEmailRequest;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ChangeEmailCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_email: Email,
}

impl IdentifiableCommand for ChangeEmailCommand {
    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn aggregate_id(&self) -> String {
        self.target.id.to_string()
    }

    fn region(&self) -> String {
        self.target.region.to_string()
    }

    fn cache_key(&self) -> Option<String> {
        Some(format!(
            "account:aggregate:{}:{}",
            self.target.region.as_str(),
            self.target.id.uuid()
        ))
    }
}

impl ChangeEmailCommand {
    pub fn try_from_proto(req: ChangeEmailRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };

        let new_email = Email::try_from(req.new_email)
            .map_err(|e| Error::validation("account_id", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            new_email,
        })
    }
}
