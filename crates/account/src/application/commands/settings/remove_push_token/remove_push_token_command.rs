// crates/account/src/application/remove_push_token/remove_push_token_command.rs

use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    security::PushToken,
    types::{AccountId, RegionCode},
};
use shared_proto::account::v1::RemovePushTokenRequest;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RemovePushTokenCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub token: PushToken,
}

impl IdentifiableCommand for RemovePushTokenCommand {
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

impl RemovePushTokenCommand {
    pub fn try_from_proto(req: RemovePushTokenRequest) -> Result<Self> {
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

        let token = PushToken::try_new(&req.token)
            .map_err(|e| Error::validation("push_token", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            token,
        })
    }
}
