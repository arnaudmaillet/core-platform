use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::Validate;

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct RevokeMfaCommand {
    pub account_id: String,
}

impl Command for RevokeMfaCommand {}
impl Validate for RevokeMfaCommand {}

pub struct RevokeMfaHandler {
    repo: Arc<dyn AccountRepository>,
}

impl RevokeMfaHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<RevokeMfaCommand> for RevokeMfaHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<RevokeMfaCommand>) -> Result<(), Self::Error> {
        let mut account = load_account(&self.repo, &envelope.payload.account_id).await?;
        account.revoke_mfa(envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
