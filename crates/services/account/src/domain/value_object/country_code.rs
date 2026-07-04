use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// ISO 3166-1 alpha-2 country code (e.g. `"US"`, `"FR"`, `"DE"`).
///
/// Always stored in uppercase. Drives compliance rules (e.g. GDPR
/// applicability for EU residents, age-of-consent thresholds).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct CountryCode(String);

impl CountryCode {
    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let s = value.into().trim().to_uppercase();

        if s.len() != 2 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(AccountError::InvalidCountryCode(format!(
                "country code must be exactly 2 ASCII letters (got {:?})",
                s
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

impl fmt::Display for CountryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
