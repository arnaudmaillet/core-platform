// shared_kernel/src/errors/infrastructure_error.rs

#[derive(Debug, thiserror::Error)]
pub enum InfrastructureError {
    #[error("Region {0} is not supported by this cluster")]
    UnsupportedRegion(String),

    #[error("No shards available for region {0}")]
    EmptyShardPool(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}