// crates/account/src/application/change_role/change_role_command.rs

use crate::domain::value_objects::AccountRole;
use serde::Deserialize;
use shared_kernel::domain::value_objects::{AccountId, AuditReason};
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct ChangeRoleCommand {
    pub command_id: Uuid,
    pub account_id: AccountId,
    pub new_role: AccountRole,
    pub reason: AuditReason,
}
