use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// Validated, canonical email address.
///
/// Always stored in lowercase. Uniquely indexed per account. The validation
/// is intentionally minimal (basic RFC 5321 structure) — full SMTP validation
/// is deferred to the email-verification flow.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EmailAddress(String);

impl EmailAddress {
    const MAX_LEN: usize = 254;

    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let raw = value.into();
        let s = raw.trim().to_lowercase();

        if s.is_empty() {
            return Err(AccountError::InvalidEmail("email must not be empty".into()));
        }
        if s.len() > Self::MAX_LEN {
            return Err(AccountError::InvalidEmail(format!(
                "email exceeds maximum length of {} characters",
                Self::MAX_LEN
            )));
        }
        if s.chars().any(|c| c.is_whitespace()) {
            return Err(AccountError::InvalidEmail(
                "email must not contain whitespace".into(),
            ));
        }

        let at_pos = s
            .rfind('@')
            .ok_or_else(|| AccountError::InvalidEmail("email must contain '@'".into()))?;

        let local = &s[..at_pos];
        let domain = &s[at_pos + 1..];

        if local.is_empty() {
            return Err(AccountError::InvalidEmail(
                "email local part must not be empty".into(),
            ));
        }
        if domain.is_empty() || !domain.contains('.') {
            return Err(AccountError::InvalidEmail(
                "email domain part must be non-empty and contain at least one dot".into(),
            ));
        }
        if domain.starts_with('.') || domain.ends_with('.') {
            return Err(AccountError::InvalidEmail(
                "email domain must not start or end with a dot".into(),
            ));
        }

        Ok(Self(s))
    }

    /// Returns a placeholder email for anonymised accounts.
    ///
    /// After GDPR erasure the real email is replaced with a deterministic
    /// placeholder derived from the account ID so the column constraints
    /// (non-null, unique index) remain satisfiable.
    pub fn anonymised(account_id: &crate::domain::value_object::account_id::AccountId) -> Self {
        Self(format!("anon-{}@deleted.invalid", account_id.as_uuid()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
