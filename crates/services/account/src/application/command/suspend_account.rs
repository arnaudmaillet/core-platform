use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct SuspendAccountCommand {
    pub account_id: String,
    /// Human-readable reason for suspension; stored for audit trail.
    pub reason: String,
}

impl Command for SuspendAccountCommand {}

impl Validate for SuspendAccountCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.reason.trim().is_empty() {
            v.push(FieldViolation::new("reason", "VAL-2040", "suspension reason must not be empty"));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct SuspendAccountHandler {
    repo: Arc<dyn AccountRepository>,
}

impl SuspendAccountHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<SuspendAccountCommand> for SuspendAccountHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<SuspendAccountCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.suspend(cmd.reason.clone(), envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
