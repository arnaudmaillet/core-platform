use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

/// Records an Art. 20 GDPR data-portability request. The data export pipeline
/// picks this up asynchronously and sets `data_export_completed_at` when done.
#[derive(Debug, Clone)]
pub struct RequestDataExportCommand {
    pub account_id: String,
}

impl Command for RequestDataExportCommand {}
impl Validate for RequestDataExportCommand {}

pub struct RequestDataExportHandler {
    repo: Arc<dyn AccountRepository>,
}

impl RequestDataExportHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<RequestDataExportCommand> for RequestDataExportHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<RequestDataExportCommand>,
    ) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.request_gdpr_data_export(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
