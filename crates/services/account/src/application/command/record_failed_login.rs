use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

/// Increments the failed-login counter; applies a timed lockout when
/// `max_attempts` is exceeded.
#[derive(Debug, Clone)]
pub struct RecordFailedLoginCommand {
    pub account_id: String,
    /// Maximum number of consecutive failures before lockout is applied.
    pub max_attempts: u16,
    /// Lockout duration in seconds once `max_attempts` is exceeded.
    pub lockout_duration_secs: u64,
}

impl Command for RecordFailedLoginCommand {}
impl Validate for RecordFailedLoginCommand {}

pub struct RecordFailedLoginHandler {
    repo: Arc<dyn AccountRepository>,
}

impl RecordFailedLoginHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<RecordFailedLoginCommand> for RecordFailedLoginHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<RecordFailedLoginCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.record_failed_login(cmd.max_attempts, cmd.lockout_duration_secs);
        self.repo.save(&account).await
    }
}
