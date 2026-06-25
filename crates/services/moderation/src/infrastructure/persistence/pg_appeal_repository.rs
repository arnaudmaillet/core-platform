use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::AppealRepository;
use crate::domain::aggregate::Appeal;
use crate::domain::value_object::AppealId;
use crate::error::ModerationError;

use super::model::AppealRow;
use super::storage_err;

/// PostgreSQL adapter for [`AppealRepository`].
#[derive(Clone)]
pub struct PgAppealRepository {
    tx: TransactionManager,
}

impl PgAppealRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl AppealRepository for PgAppealRepository {
    async fn save(&self, appeal: &Appeal) -> Result<(), ModerationError> {
        sqlx::query(
            r#"
            INSERT INTO appeals (
                id, decision_id, actor_id, statement, status, filed_at, resolved_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7)
            ON CONFLICT (id) DO UPDATE SET
                status      = EXCLUDED.status,
                resolved_at = EXCLUDED.resolved_at
            "#,
        )
        .bind(appeal.id().as_uuid())
        .bind(appeal.decision_id().as_uuid())
        .bind(appeal.actor_id().as_uuid())
        .bind(appeal.statement())
        .bind(appeal.status().as_str())
        .bind(appeal.filed_at())
        .bind(appeal.resolved_at())
        .execute(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: &AppealId) -> Result<Option<Appeal>, ModerationError> {
        let row = sqlx::query_as::<_, AppealRow>("SELECT * FROM appeals WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(storage_err)?;
        row.map(Appeal::try_from).transpose()
    }
}
