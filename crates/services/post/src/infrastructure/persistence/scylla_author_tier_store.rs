use std::sync::Arc;

use async_trait::async_trait;
use scylla::DeserializeRow;
use scylla::statement::unprepared::Statement;
use scylla_storage::{ScyllaClient, ScyllaStorageError};

use crate::application::port::AuthorTierStore;
use crate::domain::value_object::ProfileId;
use crate::error::PostError;

fn scylla_err(e: scylla::errors::ExecutionError) -> PostError {
    PostError::Storage(ScyllaStorageError::from(e))
}

fn rows_err(ctx: &'static str, e: impl ToString) -> PostError {
    PostError::DomainViolation {
        field:   ctx.to_owned(),
        message: e.to_string(),
    }
}

#[derive(DeserializeRow)]
struct TierRow {
    tier: Option<i8>,
}

/// ScyllaDB-backed `profile_id → author_tier` projection (table
/// `post.author_tiers`, single-column upsert / point read).
pub struct ScyllaAuthorTierStore {
    client: Arc<ScyllaClient>,
}

impl ScyllaAuthorTierStore {
    pub fn new(client: Arc<ScyllaClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AuthorTierStore for ScyllaAuthorTierStore {
    async fn get_tier(&self, profile_id: &ProfileId) -> Result<u8, PostError> {
        let stmt = Statement::new("SELECT tier FROM post.author_tiers WHERE profile_id = ?");
        let result = self
            .client
            .session
            .execute_unpaged(stmt, (profile_id.as_uuid(),))
            .await
            .map_err(scylla_err)?;

        let row = result
            .into_rows_result()
            .map_err(|e| rows_err("author_tier_rows", e))?
            .maybe_first_row::<TierRow>()
            .map_err(|e| rows_err("author_tier_deser", e))?;

        Ok(row.and_then(|r| r.tier).unwrap_or(0).clamp(0, 2) as u8)
    }

    async fn upsert_tier(&self, profile_id: &ProfileId, tier: u8) -> Result<(), PostError> {
        let stmt =
            Statement::new("INSERT INTO post.author_tiers (profile_id, tier) VALUES (?, ?)");
        self.client
            .session
            .execute_unpaged(stmt, (profile_id.as_uuid(), tier as i8))
            .await
            .map_err(scylla_err)?;
        Ok(())
    }
}
