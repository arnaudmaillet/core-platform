// crates/shared-kernel/src/domain/value_objects/username.rs (ou dans ton module account)

use serde::{Deserialize, Serialize};
use shared_kernel::errors::Result;
use shared_kernel::domain::value_objects::Slug;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)] // Pour que le JSON soit une string directe, pas {"0": "..."}
pub struct Username(Slug);

impl Username {
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        Ok(Self(Slug::try_new(value, "username")?))
    }

    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(Slug::from_raw(value))
    }

    pub fn as_str(&self) -> &str { self.0.as_str() }
    pub fn hash_value(&self) -> u64 { self.0.hash_value() }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}