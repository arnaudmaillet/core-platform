use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct DeactivateAccountCommand {
    pub account_id: String,
}

impl Command for DeactivateAccountCommand {}
impl Validate for DeactivateAccountCommand {}

pub struct DeactivateAccountHandler {
    repo: Arc<dyn AccountRepository>,
}

impl DeactivateAccountHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<DeactivateAccountCommand> for DeactivateAccountHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<DeactivateAccountCommand>,
    ) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.deactivate(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
