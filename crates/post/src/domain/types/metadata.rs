// crates/post/src/domain/types/dynamic_metadata.rs

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use shared_kernel::core::{Error, Result, ValueObject};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DynamicMetadata(JsonValue);

impl DynamicMetadata {
    /// Limite stricte à 64 Ko pour éviter que la ligne Cassandra ne devienne trop lourde
    pub const MAX_SIZE_BYTES: usize = 65536;

    pub fn try_new(json: JsonValue) -> Result<Self> {
        let metadata = Self(json);
        metadata.validate()?;
        Ok(metadata)
    }

    /// Initialise un objet vide `{}` (pattern Null Object) pour éviter les valeurs Option<T> pénibles
    pub fn empty() -> Self {
        Self(JsonValue::Object(serde_json::Map::new()))
    }

    /// Reconstruit l'objet depuis ScyllaDB sans ré-exécuter la validation de taille
    pub fn from_raw(json: JsonValue) -> Self {
        Self(json)
    }

    pub fn value(&self) -> &JsonValue {
        &self.0
    }

    /// Tente d'extraire et de désérialiser une feature spécifique de manière fortement typée.
    /// Exemple : let geo = metadata.get_feature::<GeoLocation>("geo_location");
    pub fn get_feature<T>(&self, key: &str) -> std::result::Result<T, serde_json::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.0.get(key) {
            Some(value) => serde_json::from_value(value.clone()),
            None => Err(serde::de::Error::custom(format!(
                "Feature '{}' not found",
                key
            ))),
        }
    }

    /// Ajoute ou met à jour une feature de manière sécurisée dans l'objet JSON
    pub fn with_feature<T>(&mut self, key: &str, feature: T) -> Result<()>
    where
        T: Serialize,
    {
        if let Some(obj) = self.0.as_object_mut() {
            let serialized = serde_json::to_value(feature).map_err(|e| {
                Error::validation("dynamic_metadata", format!("Serialization failed: {}", e))
            })?;
            obj.insert(key.to_string(), serialized);
            self.validate()?;
            Ok(())
        } else {
            Err(Error::validation(
                "dynamic_metadata",
                "Root JSON must be an object",
            ))
        }
    }
}

impl ValueObject for DynamicMetadata {
    fn validate(&self) -> Result<()> {
        // 1. On s'assure que la racine est TOUJOURS un objet JSON `{}` et pas un tableau ou une chaîne nue
        if !self.0.is_object() {
            return Err(Error::validation(
                "dynamic_metadata",
                "Dynamic metadata root must be a valid JSON object",
            ));
        }

        // 2. On valide la taille brute de la chaîne pour protéger les performances d'I/O de ScyllaDB
        let payload = serde_json::to_string(&self.0).map_err(|_| {
            Error::validation("dynamic_metadata", "Failed to calculate metadata size")
        })?;

        if payload.len() > Self::MAX_SIZE_BYTES {
            return Err(Error::validation(
                "dynamic_metadata",
                format!(
                    "Dynamic metadata payload size exceeds the limit of {} bytes",
                    Self::MAX_SIZE_BYTES
                ),
            ));
        }

        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<String> for DynamicMetadata {
    type Error = Error;
    fn try_from(s: String) -> Result<Self> {
        if s.trim().is_empty() {
            return Ok(Self::empty());
        }
        let json: JsonValue = serde_json::from_str(&s)
            .map_err(|_| Error::validation("dynamic_metadata", "Invalid JSON format string"))?;
        Self::try_new(json)
    }
}

impl From<DynamicMetadata> for String {
    fn from(metadata: DynamicMetadata) -> Self {
        serde_json::to_string(&metadata.0).unwrap_or_else(|_| "{}".to_string())
    }
}

impl FromStr for DynamicMetadata {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        Self::try_from(s.to_string())
    }
}

impl std::fmt::Display for DynamicMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(&self.0).unwrap_or_else(|_| "{}".to_string())
        )
    }
}
