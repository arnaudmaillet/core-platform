use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::AccountRole;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct AssignRoleCommand {
    pub account_id: String,
    /// Serialised `AccountRole` variant name (e.g. `"ContentModerator"`).
    pub role: String,
}

impl Command for AssignRoleCommand {}

impl Validate for AssignRoleCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.role.trim().is_empty() {
            v.push(FieldViolation::new("role", "VAL-2060", "role must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct AssignRoleHandler {
    repo: Arc<dyn AccountRepository>,
}

impl AssignRoleHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<AssignRoleCommand> for AssignRoleHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<AssignRoleCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let role = AccountRole::try_from(cmd.role.trim())
            .map_err(|_| AccountError::InvalidAccountRole(cmd.role.clone()))?;

        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.assign_role(role, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
