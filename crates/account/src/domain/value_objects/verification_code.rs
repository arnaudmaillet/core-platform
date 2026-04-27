use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::ValueObject;
use shared_kernel::errors::{DomainError, Result};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VerificationCode {
    inner: String,
}

impl VerificationCode {
    /// Longueur standard pour un OTP (One-Time Password)
    pub const LENGTH: usize = 6;

    /// Constructeur avec nettoyage et validation
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw = value.into();

        // 1. Nettoyage : On ne garde que les chiffres (enlève espaces, tirets, etc.)
        let cleaned: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();

        let code = Self { inner: cleaned };

        // 2. Validation
        code.validate()?;

        Ok(code)
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }
}

impl ValueObject for VerificationCode {
    fn validate(&self) -> Result<()> {
        // 1. Vérification de la longueur
        if self.inner.len() != Self::LENGTH {
            return Err(DomainError::Validation {
                field: "verification_code",
                reason: format!("Must be exactly {} digits", Self::LENGTH).into(),
            });
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for VerificationCode {
    type Error = DomainError;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<VerificationCode> for String {
    fn from(code: VerificationCode) -> Self {
        code.inner
    }
}

impl fmt::Display for VerificationCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.inner)
    }
}
