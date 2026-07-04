use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct VerifyEmailCommand {
    pub account_id: String,
}

impl Command for VerifyEmailCommand {}
impl Validate for VerifyEmailCommand {}

pub struct VerifyEmailHandler {
    repo: Arc<dyn AccountRepository>,
}

impl VerifyEmailHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<VerifyEmailCommand> for VerifyEmailHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<VerifyEmailCommand>) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.verify_email(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
