// crates/account/src/application/commands/access_management.rs (ou ton sous-module dédié)

use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{AccountId, Region};
use shared_proto::account::v1::VerifyPhoneRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct VerifyPhoneCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub code: String,
}

impl IdentifiableCommand for VerifyPhoneCommand {
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

impl VerifyPhoneCommand {
    pub fn try_from_proto(req: VerifyPhoneRequest, region: Region) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let code = req.code.trim();
        if code.is_empty() {
            return Err(Error::validation(
                "code",
                "Verification code cannot be empty",
            ));
        }

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        Ok(Self {
            command_id,
            target,
            region,
            code: code.to_string(),
        })
    }
}
