// crates/shared_kernel/src/domain/value_objects/url.rs

use crate::core::{Error, Result, ValueObject};
use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url as LibUrl;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Url(String);

impl Url {
    /// Constructeur sécurisé (Domaine / API)
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        let raw_string = value.into();

        // 1. Parsing via la crate 'url' pour normalisation syntaxique
        let parsed = LibUrl::parse(&raw_string)
            .map_err(|_| Error::validation("url", format!("Invalid URL format: {}", raw_string)))?;

        // 2. Création de l'instance
        let url = Self(parsed.to_string());

        // 3. Validation métier stricte
        url.validate()?;

        Ok(url)
    }

    /// Reconstruction rapide (Infrastructure / DB)
    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for Url {
    fn validate(&self) -> Result<()> {
        let parsed =
            LibUrl::parse(&self.0).map_err(|_| Error::validation("url", "Invalid URL state"))?;

        // Sécurité Hyperscale : On restreint les protocoles
        let scheme = parsed.scheme();
        if scheme != "http" && scheme != "https" {
            return Err(Error::validation(
                "url",
                "Only http and https protocols are allowed",
            ));
        }

        Ok(())
    }
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Url {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}
