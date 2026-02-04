// crates/shared-kernel/src/error/context.rs
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize, Clone)]
pub struct ErrorContext {
    /// Nom du champ en erreur (ex: "email")
    pub field: String,
    /// Message d'erreur spécifique au champ
    pub message: String,
    /// Métadonnées supplémentaires (ex: { "min": 8 })
    pub metadata: Option<HashMap<String, String>>,
}

impl ErrorContext {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
            metadata: None,
        }
    }
}
