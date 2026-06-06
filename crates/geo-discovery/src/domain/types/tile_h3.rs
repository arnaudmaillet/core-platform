use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TileH3(String);

impl TileH3 {
    pub fn try_new(index_str: String) -> Result<Self> {
        let tile = Self(index_str.trim().to_lowercase());
        tile.validate()?;
        Ok(tile)
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

impl ValueObject for TileH3 {
    fn validate(&self) -> Result<()> {
        if self.0.is_empty() {
            return Err(Error::validation(
                "h3_tile",
                "H3 tile string cannot be empty",
            ));
        }
        // Un index H3 valide sous forme de chaîne hexadécimale fait généralement 15 caractères
        if self.0.len() < 14 || self.0.len() > 15 {
            return Err(Error::validation(
                "h3_tile",
                "Invalid hexadecimal H3 index length",
            ));
        }
        if !self.0.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Error::validation(
                "h3_tile",
                "H3 index must be a valid hexadecimal string",
            ));
        }
        Ok(())
    }
}

impl TryFrom<String> for TileH3 {
    type Error = Error;
    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

impl From<TileH3> for String {
    fn from(tile: TileH3) -> Self {
        tile.0
    }
}

impl FromStr for TileH3 {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_new(s.to_string())
    }
}

impl fmt::Display for TileH3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.value())
    }
}
