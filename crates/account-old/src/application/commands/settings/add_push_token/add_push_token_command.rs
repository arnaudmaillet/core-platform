use shared_kernel::command::{CommandTarget, IdentifiableCommand};
// crates/account/src/application/add_push_token/add_push_token_command.rs
use shared_kernel::core::{Error, Result};
use shared_kernel::security::PushToken;
use shared_kernel::types::{AccountId, Region};
use shared_proto::account::v1::AddPushTokenRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AddPushTokenCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub region: Region,
    pub token: PushToken,
}

impl IdentifiableCommand for AddPushTokenCommand {
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

impl AddPushTokenCommand {
    pub fn try_from_proto(req: AddPushTokenRequest, region: Region) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            expected_version: Some(proto_target.expected_version),
        };

        let token = PushToken::try_new(&req.token)
            .map_err(|e| Error::validation("push_token", e.to_string()))?;


        Ok(Self {
            command_id,
            target,
            region,
            token,
        })
    }
}
