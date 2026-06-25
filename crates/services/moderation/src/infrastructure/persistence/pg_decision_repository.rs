use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::DecisionRepository;
use crate::domain::aggregate::Decision;
use crate::domain::value_object::DecisionId;
use crate::error::ModerationError;

use super::model::DecisionRow;
use super::storage_err;

/// PostgreSQL adapter for the append-only [`DecisionRepository`].
#[derive(Clone)]
pub struct PgDecisionRepository {
    tx: TransactionManager,
}

impl PgDecisionRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl DecisionRepository for PgDecisionRepository {
    async fn append(&self, decision: &Decision) -> Result<(), ModerationError> {
        let subject = decision.subject();
        let author_kind = if decision.is_automated() { "rule" } else { "reviewer" };
        // Append-only: a fresh UUIDv7 id never collides; DO NOTHING guards a benign
        // re-append (e.g. an at-least-once retry) without ever mutating the record.
        sqlx::query(
            r#"
            INSERT INTO decisions (
                id, entity_type, entity_id, actor_id, surface, action, category,
                policy_version, rationale, author_kind, author_id, reverses, decided_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(decision.id().as_uuid())
        .bind(subject.entity_type().as_str())
        .bind(subject.entity_id())
        .bind(subject.actor_id().as_uuid())
        .bind(subject.surface())
        .bind(decision.action().as_str())
        .bind(decision.category().as_str())
        .bind(decision.policy_version().as_str())
        .bind(decision.rationale())
        .bind(author_kind)
        .bind(decision.author().id())
        .bind(decision.reverses().map(|d| d.as_uuid()))
        .bind(decision.decided_at())
        .execute(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: &DecisionId) -> Result<Option<Decision>, ModerationError> {
        let row = sqlx::query_as::<_, DecisionRow>("SELECT * FROM decisions WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(storage_err)?;
        row.map(Decision::try_from).transpose()
    }
}
