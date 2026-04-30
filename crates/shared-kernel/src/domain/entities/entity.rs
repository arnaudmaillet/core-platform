// crates/shared-kernel/src/domain/entity.rs
use crate::domain::Identifier;
use crate::errors::DomainError;
use chrono::{DateTime, Utc};

pub trait Entity {
    type Id: Identifier;

    // --- Métadonnées (statiques) ---
    fn entity_name() -> &'static str;

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "unique_constraint"
    }

    // --- Accès aux données (instance) ---
    fn id(&self) -> &Self::Id;
    fn updated_at(&self) -> DateTime<Utc>;

    // --- Helper par défaut ---
    fn not_found<I: ToString>(id: I) -> DomainError {
        DomainError::NotFound {
            entity: Self::entity_name(),
            id: id.to_string(),
        }
    }
}

pub trait EntityOptionExt<T> {
    fn ok_or_not_found<I: ToString>(self, id: I) -> Result<T, DomainError>
    where
        T: Entity;
}

impl<T> EntityOptionExt<T> for Option<T> {
    fn ok_or_not_found<I: ToString>(self, id: I) -> Result<T, DomainError>
    where
        T: Entity,
    {
        self.ok_or_else(|| T::not_found(id))
    }
}
