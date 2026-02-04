// crates/shared-kernel/src/infrastructure/postgres/mappers/postgres_error_mapper.rs

use crate::domain::entities::EntityMetadata;
use crate::errors::DomainError;
use sqlx::postgres::PgDatabaseError;

pub trait SqlxErrorExt<T> {
    fn map_domain<E: EntityMetadata>(self) -> Result<T, DomainError>;
    fn map_domain_infra(self, context: &'static str) -> Result<T, DomainError>;
}

impl<T> SqlxErrorExt<T> for std::result::Result<T, sqlx::Error> {
    fn map_domain<E: EntityMetadata>(self) -> Result<T, DomainError> {
        self.map_err(|e| {
            match e {
                sqlx::Error::RowNotFound => DomainError::NotFound {
                    entity: E::entity_name(),
                    id: "unknown".into(),
                },
                sqlx::Error::Database(db_err) => {
                    // 1. Violation d'unicité (Code Postgres 23505)
                    if db_err.code().map(|c| c == "23505").unwrap_or(false) {
                        let mut field = "unique_constraint";

                        if let Some(constraint_name) = db_err.try_downcast_ref::<PgDatabaseError>().and_then(|pg| pg.constraint()) {
                            field = E::map_constraint_to_field(constraint_name);
                        }

                        return DomainError::AlreadyExists {
                            entity: E::entity_name(),
                            field,
                            value: "already taken".into(),
                        };
                    }

                    // 2. Concurrence (Code Postgres 40001)
                    if db_err.code().map(|c| c == "40001").unwrap_or(false) {
                        return DomainError::ConcurrencyConflict {
                            reason: format!("Concurrency conflict on {}", E::entity_name()),
                        };
                    }

                    DomainError::Infrastructure(db_err.message().into())
                }
                _ => DomainError::Infrastructure(e.to_string()),
            }
        })
    }

    fn map_domain_infra(self, context: &'static str) -> Result<T, DomainError> {
        self.map_err(|e| DomainError::Infrastructure(format!("{}: {}", context, e)))
    }
}
