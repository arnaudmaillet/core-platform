use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::geo::Timezone;
use shared_kernel::types::{AccountId, Region};
use shared_proto::account::v1::UpdateTimezoneRequest;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTimezoneCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_timezone: Timezone,
}

impl IdentifiableCommand for UpdateTimezoneCommand {
    type Id = AccountId;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<AccountId> {
        &self.target
    }
}

impl UpdateTimezoneCommand {
    pub fn try_from_proto(req: UpdateTimezoneRequest) -> Result<Self> {
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

        let new_timezone = Timezone::try_new(&req.timezone)
            .map_err(|e| Error::validation("timezone", e.to_string()))?;

        Ok(Self {
            command_id,
            target,
            new_timezone,
        })
    }
}
