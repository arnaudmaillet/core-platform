use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::AccountRole;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct RevokeRoleCommand {
    pub account_id: String,
    pub role: String,
}

impl Command for RevokeRoleCommand {}

impl Validate for RevokeRoleCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.role.trim().is_empty() {
            v.push(FieldViolation::new("role", "VAL-2070", "role must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct RevokeRoleHandler {
    repo: Arc<dyn AccountRepository>,
}

impl RevokeRoleHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<RevokeRoleCommand> for RevokeRoleHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<RevokeRoleCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;

        let role = AccountRole::try_from(cmd.role.trim())
            .map_err(|_| AccountError::InvalidAccountRole(cmd.role.clone()))?;

        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.revoke_role(role, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
