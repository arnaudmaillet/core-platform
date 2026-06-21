use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

/// Records an Art. 17 GDPR right-to-erasure request and schedules the
/// anonymisation deadline at `retention_days` from now.
#[derive(Debug, Clone)]
pub struct RequestGdprDeletionCommand {
    pub account_id: String,
    /// Minimum number of days before PII is scrubbed (legal retention minimum).
    pub retention_days: u32,
}

impl Command for RequestGdprDeletionCommand {}
impl Validate for RequestGdprDeletionCommand {}

pub struct RequestGdprDeletionHandler {
    repo: Arc<dyn AccountRepository>,
}

impl RequestGdprDeletionHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<RequestGdprDeletionCommand> for RequestGdprDeletionHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<RequestGdprDeletionCommand>,
    ) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        account.request_gdpr_deletion(cmd.retention_days, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
