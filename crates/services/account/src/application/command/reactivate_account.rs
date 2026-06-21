use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct ReactivateAccountCommand {
    pub account_id: String,
}

impl Command for ReactivateAccountCommand {}
impl Validate for ReactivateAccountCommand {}

pub struct ReactivateAccountHandler {
    repo: Arc<dyn AccountRepository>,
}

impl ReactivateAccountHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<ReactivateAccountCommand> for ReactivateAccountHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<ReactivateAccountCommand>,
    ) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.activate(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
