// crates/shared-kernel/src/persistence/postgres/repositories/idempotency.rs

use crate::TransactionExt;
use async_trait::async_trait;
use shared_kernel::core::{Error, Result, Transaction};
use shared_kernel::idempotency::IdempotencyRepository;
use uuid::Uuid;

const EXISTS_QUERY: &str = r#"
    SELECT EXISTS(
        SELECT 1 FROM processed_commands 
        WHERE command_id = $1 AND namespace = $2
    )
"#;

const SAVE_QUERY: &str = r#"
    INSERT INTO processed_commands (command_id, namespace, occurred_at)
    VALUES ($1, $2, NOW())
    ON CONFLICT (command_id, namespace) DO NOTHING
"#;

pub struct PostgresIdempotencyRepository {
    namespace: String,
}

impl PostgresIdempotencyRepository {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
        }
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresIdempotencyRepository {
    async fn exists(
        &self,
        tx: Option<&mut (dyn Transaction + '_)>,
        command_id: &Uuid,
    ) -> Result<bool> {
        let tx_ref = tx.ok_or_else(|| {
            Error::internal("PostgresIdempotencyRepository requires an active transaction context")
        })?;

        let sqlx_tx = tx_ref.downcast_mut_sqlx()?;

        let row: (bool,) = sqlx::query_as(EXISTS_QUERY)
            .bind(command_id)
            .bind(&self.namespace)
            .fetch_one(&mut **sqlx_tx)
            .await
            .map_err(|e| Error::database(format!("Idempotency check failed: {}", e)))?;

        Ok(row.0)
    }

    async fn save(&self, tx: Option<&mut (dyn Transaction + '_)>, command_id: &Uuid) -> Result<()> {
        let tx_ref = tx.ok_or_else(|| {
            Error::internal("PostgresIdempotencyRepository requires an active transaction context")
        })?;

        let sqlx_tx = tx_ref.downcast_mut_sqlx()?;

        sqlx::query(SAVE_QUERY)
            .bind(command_id)
            .bind(&self.namespace)
            .execute(&mut **sqlx_tx)
            .await
            .map_err(|e| Error::database(format!("Idempotency save failed: {}", e)))?;

        Ok(())
    }
}
