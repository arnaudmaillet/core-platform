use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::PasswordHash;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct ChangePasswordCommand {
    pub account_id: String,
    /// Argon2id hash of the new password; must be non-empty.
    pub new_password_hash: String,
}

impl Command for ChangePasswordCommand {}

impl Validate for ChangePasswordCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.new_password_hash.trim().is_empty() {
            v.push(FieldViolation::new(
                "new_password_hash",
                "VAL-2010",
                "new_password_hash must not be empty",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct ChangePasswordHandler {
    repo: Arc<dyn AccountRepository>,
}

impl ChangePasswordHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<ChangePasswordCommand> for ChangePasswordHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<ChangePasswordCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let mut account = load_account(&self.repo, &cmd.account_id).await?;
        let hash = PasswordHash::from_hash(cmd.new_password_hash.clone());
        account.change_password(hash, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
