use async_trait::async_trait;
use postgres_storage::TransactionManager;
use sqlx::types::Json;

use crate::application::port::PenaltyRepository;
use crate::domain::aggregate::PenaltyLedger;
use crate::domain::value_object::ActorId;
use crate::error::ModerationError;

use super::model::PenaltyRow;
use super::storage_err;

/// PostgreSQL adapter for [`PenaltyRepository`]. One row per actor; the strike
/// history is stored as JSON and evaluated in-domain.
#[derive(Clone)]
pub struct PgPenaltyRepository {
    tx: TransactionManager,
}

impl PgPenaltyRepository {
    pub fn new(tx: TransactionManager) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl PenaltyRepository for PgPenaltyRepository {
    async fn load(&self, actor_id: &ActorId) -> Result<PenaltyLedger, ModerationError> {
        let row =
            sqlx::query_as::<_, PenaltyRow>("SELECT * FROM penalty_ledgers WHERE actor_id = $1")
                .bind(actor_id.as_uuid())
                .fetch_optional(self.tx.pool())
                .await
                .map_err(storage_err)?;
        Ok(row.map(PenaltyLedger::from).unwrap_or_else(|| PenaltyLedger::empty(*actor_id)))
    }

    async fn save(&self, ledger: &PenaltyLedger) -> Result<(), ModerationError> {
        sqlx::query(
            r#"
            INSERT INTO penalty_ledgers (actor_id, strikes, version)
            VALUES ($1, $2, $3)
            ON CONFLICT (actor_id) DO UPDATE SET
                strikes = EXCLUDED.strikes,
                version = EXCLUDED.version
            "#,
        )
        .bind(ledger.actor_id().as_uuid())
        .bind(Json(ledger.strikes().to_vec()))
        .bind(ledger.version())
        .execute(self.tx.pool())
        .await
        .map_err(storage_err)?;
        Ok(())
    }
}
