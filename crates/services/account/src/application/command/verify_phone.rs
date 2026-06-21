use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct VerifyPhoneCommand {
    pub account_id: String,
}

impl Command for VerifyPhoneCommand {}
impl Validate for VerifyPhoneCommand {}

pub struct VerifyPhoneHandler {
    repo: Arc<dyn AccountRepository>,
}

impl VerifyPhoneHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<VerifyPhoneCommand> for VerifyPhoneHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<VerifyPhoneCommand>) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.verify_phone(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
