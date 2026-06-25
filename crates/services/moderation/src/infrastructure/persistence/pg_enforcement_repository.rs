use async_trait::async_trait;
use postgres_storage::TransactionManager;

use crate::application::port::EnforcementRepository;
use crate::domain::aggregate::EnforcementAction;
use crate::domain::value_object::{ActorId, EnforcementId, EnforcementVersion, SubjectRef};
use crate::error::ModerationError;

use super::model::EnforcementRow;
use super::storage_err;

/// PostgreSQL adapter for [`EnforcementRepository`].
#[derive(Clone)]
pub struct PgEnforcementRepository {
    tx: TransactionManager,
}

impl PgEnforcementRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl EnforcementRepository for PgEnforcementRepository {
    async fn save(&self, enforcement: &EnforcementAction) -> Result<(), ModerationError> {
        let subject = enforcement.subject();
        sqlx::query(
            r#"
            INSERT INTO enforcements (
                id, entity_type, entity_id, actor_id, surface, action, status,
                version, decision_id, applied_at, expires_at, reversed_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            ON CONFLICT (id) DO UPDATE SET
                status      = EXCLUDED.status,
                reversed_at = EXCLUDED.reversed_at
            "#,
        )
        .bind(enforcement.id().as_uuid())
        .bind(subject.entity_type().as_str())
        .bind(subject.entity_id())
        .bind(subject.actor_id().as_uuid())
        .bind(subject.surface())
        .bind(enforcement.action().as_str())
        .bind(enforcement.status().as_str())
        .bind(enforcement.version().value())
        .bind(enforcement.decision_id().as_uuid())
        .bind(enforcement.applied_at())
        .bind(enforcement.expires_at())
        .bind(enforcement.reversed_at())
        .execute(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(())
    }

    async fn find_by_id(
        &self,
        id: &EnforcementId,
    ) -> Result<Option<EnforcementAction>, ModerationError> {
        let row = sqlx::query_as::<_, EnforcementRow>("SELECT * FROM enforcements WHERE id = $1")
            .bind(id.as_uuid())
            .fetch_optional(self.tx.pool())
            .await
            .map_err(storage_err)?;
        row.map(EnforcementAction::try_from).transpose()
    }

    async fn next_version(
        &self,
        subject: &SubjectRef,
    ) -> Result<EnforcementVersion, ModerationError> {
        // MAX(version) over the subject's enforcements; 0 when none ⇒ next() = INITIAL.
        let max: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT COALESCE(MAX(version), 0) FROM enforcements
            WHERE entity_type = $1 AND entity_id = $2 AND surface = $3
            "#,
        )
        .bind(subject.entity_type().as_str())
        .bind(subject.entity_id())
        .bind(subject.surface())
        .fetch_one(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(EnforcementVersion::from_i64(max.unwrap_or(0)).next())
    }

    async fn list_active_for_actor(
        &self,
        actor_id: &ActorId,
    ) -> Result<Vec<EnforcementAction>, ModerationError> {
        let rows = sqlx::query_as::<_, EnforcementRow>(
            "SELECT * FROM enforcements WHERE actor_id = $1 AND status = 'active'",
        )
        .bind(actor_id.as_uuid())
        .fetch_all(self.tx.pool())
        .await
        .map_err(storage_err)?;
        rows.into_iter().map(EnforcementAction::try_from).collect()
    }
}
