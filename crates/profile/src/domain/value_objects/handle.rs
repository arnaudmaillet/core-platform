// crates/profile/src/domain/value_objects/handle.rs

use serde::{Deserialize, Serialize};
use shared_kernel::domain::value_objects::Slug;
use shared_kernel::errors::Result;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Handle(Slug);

impl Handle {
    pub fn try_new(value: impl Into<String>) -> Result<Self> {
        Ok(Self(Slug::try_new(value, "handle")?))
    }

    pub fn from_raw(value: impl Into<String>) -> Self {
        Self(Slug::from_raw(value))
    }

    pub fn as_str(&self) -> &str { self.0.as_str() }
    pub fn hash_value(&self) -> u64 { self.0.hash_value() }
}

// --- AJOUTS CI-DESSOUS ---

/// Permet la conversion faillible (ex: depuis une API ou un input utilisateur)
impl TryFrom<String> for Handle {
    type Error = shared_kernel::errors::DomainError;

    fn try_from(value: String) -> Result<Self> {
        Self::try_new(value)
    }
}

/// Permet d'utiliser .into() pour transformer un Handle en String
impl From<Handle> for String {
    fn from(handle: Handle) -> Self {
        handle.to_string()
    }
}

impl std::fmt::Display for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}