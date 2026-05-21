// crates/shared-kernel/src/persistence/postgres/repositories/idempotency.rs

use crate::core::{Error, Result, Transaction};
use crate::idempotency::IdempotencyRepository;
use crate::postgres::TransactionExt;
use async_trait::async_trait;
use uuid::Uuid;

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

        let sql = "SELECT EXISTS(SELECT 1 FROM processed_commands WHERE command_id = $1 AND namespace = $2)";

        let row: (bool,) = sqlx::query_as(sql)
            .bind(command_id)
            .bind(&self.namespace)
            .fetch_one(&mut **sqlx_tx)
            .await
            .map_err(|e| Error::database(format!("Idempotency check failed: {}", e.to_string())))?;

        Ok(row.0)
    }

    async fn save(&self, tx: Option<&mut (dyn Transaction + '_)>, command_id: &Uuid) -> Result<()> {
        let tx_ref = tx.ok_or_else(|| {
            Error::internal("PostgresIdempotencyRepository requires an active transaction context")
        })?;

        let sqlx_tx = tx_ref.downcast_mut_sqlx()?;

        let sql = r#"
            INSERT INTO processed_commands (command_id, namespace, occurred_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (command_id, namespace) DO NOTHING
        "#;

        sqlx::query(sql)
            .bind(command_id)
            .bind(&self.namespace)
            .execute(&mut **sqlx_tx)
            .await
            .map_err(|e| Error::database(format!("Idempotency save failed: {}", e.to_string())))?;

        Ok(())
    }
}
