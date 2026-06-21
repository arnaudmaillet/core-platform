use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// E.164-formatted international phone number.
///
/// Format: `+` followed by 7–14 digits, for a total length of 8–15 characters.
/// The `+` country-code prefix is mandatory; no spaces, dashes, or parentheses.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    /// Minimum total length including the leading `+`.
    const MIN_LEN: usize = 8;
    /// Maximum total length including the leading `+`.
    const MAX_LEN: usize = 15;

    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let s = value.into();

        if !s.starts_with('+') {
            return Err(AccountError::InvalidPhone(
                "phone number must start with '+' (E.164 format)".into(),
            ));
        }

        let digits = &s[1..];
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(AccountError::InvalidPhone(
                "phone number must contain only digits after '+'".into(),
            ));
        }

        let len = s.len();
        if len < Self::MIN_LEN || len > Self::MAX_LEN {
            return Err(AccountError::InvalidPhone(format!(
                "phone number length must be between {} and {} characters (got {})",
                Self::MIN_LEN,
                Self::MAX_LEN,
                len
            )));
        }

        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for PhoneNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
