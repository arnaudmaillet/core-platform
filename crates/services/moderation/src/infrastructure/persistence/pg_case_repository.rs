use async_trait::async_trait;
use postgres_storage::TransactionManager;
use sqlx::types::Json;

use crate::application::port::CaseRepository;
use crate::domain::aggregate::Case;
use crate::domain::value_object::{CaseId, CaseStatus};
use crate::error::ModerationError;

use super::model::CaseRow;
use super::storage_err;

/// PostgreSQL adapter for [`CaseRepository`].
#[derive(Clone)]
pub struct PgCaseRepository {
    tx: TransactionManager,
}

impl PgCaseRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl CaseRepository for PgCaseRepository {
    async fn save(&self, case: &Case) -> Result<(), ModerationError> {
        let subject = case.subject();
        sqlx::query(
            r#"
            INSERT INTO cases (
                id, entity_type, entity_id, actor_id, surface, status, category,
                queue, priority, assignee, signals, opened_at, version
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            ON CONFLICT (id) DO UPDATE SET
                status   = EXCLUDED.status,
                category = EXCLUDED.category,
                queue    = EXCLUDED.queue,
                priority = EXCLUDED.priority,
                assignee = EXCLUDED.assignee,
                signals  = EXCLUDED.signals,
                version  = EXCLUDED.version
            "#,
        )
        .bind(case.id().as_uuid())
        .bind(subject.entity_type().as_str())
        .bind(subject.entity_id())
        .bind(subject.actor_id().as_uuid())
        .bind(subject.surface())
        .bind(case.status().as_str())
        .bind(case.category().as_str())
        .bind(case.queue())
        .bind(case.priority())
        .bind(case.assignee())
        .bind(Json(case.signals().to_vec()))
        .bind(case.opened_at())
        .bind(case.version())
        .execute(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: &CaseId) -> Result<Option<Case>, ModerationError> {
        let row = sqlx::query_as::<_, CaseRow>("SELECT * FROM cases WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(storage_err)?;
        row.map(Case::try_from).transpose()
    }

    async fn list_queue(
        &self,
        queue: &str,
        status: Option<CaseStatus>,
        limit: usize,
    ) -> Result<Vec<Case>, ModerationError> {
        let status_filter = status.map(|s| s.as_str().to_owned());
        let rows = sqlx::query_as::<_, CaseRow>(
            r#"
            SELECT * FROM cases
            WHERE queue = $1 AND ($2::text IS NULL OR status = $2)
            ORDER BY opened_at DESC
            LIMIT $3
            "#,
        )
        .bind(queue)
        .bind(status_filter)
        .bind(limit as i64)
        .fetch_all(self.tx.pool())
        .await
        .map_err(storage_err)?;
        rows.into_iter().map(Case::try_from).collect()
    }
}
