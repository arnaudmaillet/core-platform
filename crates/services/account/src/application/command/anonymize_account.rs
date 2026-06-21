use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

/// Executes the PII scrub: nullifies personal data and marks the account as
/// anonymised. Triggered by the GDPR janitor worker after `deletion_scheduled_at`
/// has elapsed.
#[derive(Debug, Clone)]
pub struct AnonymizeAccountCommand {
    pub account_id: String,
}

impl Command for AnonymizeAccountCommand {}
impl Validate for AnonymizeAccountCommand {}

pub struct AnonymizeAccountHandler {
    repo: Arc<dyn AccountRepository>,
}

impl AnonymizeAccountHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<AnonymizeAccountCommand> for AnonymizeAccountHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<AnonymizeAccountCommand>) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.anonymize(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
