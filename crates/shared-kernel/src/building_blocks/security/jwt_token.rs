use crate::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct JwtToken(String);

impl JwtToken {
    /// Un JWT a au minimum 3 parties séparées par des points (header.payload.signature)
    pub const MIN_LENGTH: usize = 16;
    /// Un JWT peut être assez long selon les claims inclus
    pub const MAX_LENGTH: usize = 8192;

    pub fn try_new(token: impl Into<String>) -> Result<Self> {
        let raw = token.into();
        let trimmed = raw.trim().to_string();
        let jwt = Self(trimmed);
        jwt.validate()?;
        Ok(jwt)
    }

    pub fn from_raw(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for JwtToken {
    fn validate(&self) -> Result<()> {
        let len = self.0.len();

        if len < Self::MIN_LENGTH || len > Self::MAX_LENGTH {
            return Err(Error::validation("jwt_token", "Invalid JWT length"));
        }

        // Vérification basique du format JWT (doit contenir au moins deux '.')
        if self.0.chars().filter(|&c| c == '.').count() < 2 {
            return Err(Error::validation(
                "jwt_token",
                "Malformed JWT: missing parts",
            ));
        }

        Ok(())
    }
}

impl fmt::Display for JwtToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Optionnel : masquer une partie du token pour les logs
        write!(f, "{}...", &self.0[..4.min(self.0.len())])
    }
}
