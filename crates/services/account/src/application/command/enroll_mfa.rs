use std::sync::Arc;

use cqrs::{Command, CommandHandler, Envelope};
use validate_core::{FieldViolation, Validate};

use crate::application::command::helpers::load_account;
use crate::application::port::AccountRepository;
use crate::domain::value_object::{EncryptedBytes, RecoveryCodeHash};
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct EnrollMfaCommand {
    pub account_id: String,
    /// AES-256-GCM ciphertext of the TOTP seed; key managed by KMS.
    pub totp_secret_ciphertext: Vec<u8>,
    /// Bcrypt-hashed one-time recovery codes; at least 6 required.
    pub recovery_code_hashes: Vec<String>,
}

impl Command for EnrollMfaCommand {}

impl Validate for EnrollMfaCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.totp_secret_ciphertext.is_empty() {
            v.push(FieldViolation::new(
                "totp_secret_ciphertext",
                "VAL-2020",
                "TOTP secret ciphertext must not be empty",
            ));
        }
        if self.recovery_code_hashes.len() < 6 {
            v.push(FieldViolation::new(
                "recovery_code_hashes",
                "VAL-2021",
                "at least 6 recovery code hashes are required",
            ));
        }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

pub struct EnrollMfaHandler {
    repo: Arc<dyn AccountRepository>,
}

impl EnrollMfaHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CommandHandler<EnrollMfaCommand> for EnrollMfaHandler {
    type Error = AccountError;

    async fn handle(&self, envelope: Envelope<EnrollMfaCommand>) -> Result<(), Self::Error> {
        let cmd = &envelope.payload;
        let mut account = load_account(&self.repo, &cmd.account_id).await?;

        let secret = EncryptedBytes::from_ciphertext(cmd.totp_secret_ciphertext.clone());
        let codes: Vec<RecoveryCodeHash> =
            cmd.recovery_code_hashes.iter().map(|h| RecoveryCodeHash::from_hash(h.clone())).collect();

        account.enroll_mfa(secret, codes, envelope.correlation_id)?;
        self.repo.save(&account).await
    }
}
