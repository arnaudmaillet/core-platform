// crates/shared-kernel/src/domain/entity.rs
use chrono::{DateTime, Utc};
use crate::domain::Identifier;
use crate::errors::DomainError;

pub trait EntityMetadata {
    fn entity_name() -> &'static str;

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "unique_constraint"
    }

    fn not_found<I: ToString>(id: I) -> DomainError {
        DomainError::NotFound {
            entity: Self::entity_name(),
            id: id.to_string(),
        }
    }
}

pub trait Entity: EntityMetadata {
    type Id: Identifier;

    fn id(&self) -> &Self::Id;
    fn created_at(&self) -> DateTime<Utc>;
    fn updated_at(&self) -> Option<DateTime<Utc>>;

}

pub trait EntityOptionExt<T> {
    fn ok_or_not_found<I: ToString>(self, id: I) -> Result<T, DomainError>
    where T: EntityMetadata;
}

impl<T> EntityOptionExt<T> for Option<T> {
    fn ok_or_not_found<I: ToString>(self, id: I) -> Result<T, DomainError>
    where T: EntityMetadata
    {
        self.ok_or_else(|| T::not_found(id))
    }
}