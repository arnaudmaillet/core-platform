// crates/account/src/application/change_role/change_role_command.rs

use crate::domain::types::AccountRole;
use serde::Deserialize;
use shared_kernel::{
    command::{CommandTarget, IdentifiableCommand},
    core::{Error, Result},
    types::{AccountId, AuditReason, Region},
};
use shared_proto::account::v1::ChangeRoleRequest;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRoleCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<AccountId>,
    pub new_role: AccountRole,
    pub reason: AuditReason,
}

impl IdentifiableCommand for ChangeRoleCommand {
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

impl ChangeRoleCommand {
    pub fn try_from_proto(req: ChangeRoleRequest) -> Result<Self> {
        let proto_target = req
            .target
            .ok_or_else(|| Error::validation("target", "Missing profile target"))?;

        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID format"))?;

        let reason = AuditReason::try_from(req.reason)
            .map_err(|e| Error::validation("reason", e.to_string()))?;

        let new_role = AccountRole::try_from(req.new_role)
            .map_err(|e| Error::validation("new_role", e.to_string()))?;

        let target = CommandTarget {
            id: AccountId::try_from(proto_target.account_id)?,
            region: Region::try_new(proto_target.region)?,
            expected_version: proto_target.expected_version,
        };
        Ok(Self {
            command_id,
            target,
            new_role,
            reason,
        })
    }
}
