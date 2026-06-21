use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AccountError;

/// ISO 4217 currency code (e.g. `"USD"`, `"EUR"`, `"GBP"`).
///
/// Always stored in uppercase. Immutable once assigned to a
/// [`CreditLedger`] — currency changes require a separate
/// migration operation that is out of scope for this service.
///
/// [`CreditLedger`]: crate::domain::entity::credit_ledger::CreditLedger
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct CurrencyCode(String);

impl CurrencyCode {
    pub fn new(value: impl Into<String>) -> Result<Self, AccountError> {
        let s = value.into().trim().to_uppercase();

        if s.len() != 3 || !s.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(AccountError::InvalidCurrencyCode(format!(
                "currency code must be exactly 3 ASCII letters (got {:?})",
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

impl fmt::Display for CurrencyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
