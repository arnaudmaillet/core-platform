use crate::TransactionExt;
use async_trait::async_trait;
use shared_kernel::core::{Error, Result, Transaction};
use shared_kernel::idempotency::IdempotencyRepository;
use sqlx::{Pool, Postgres}; // 💡 Ajout de l'import du Pool SQLx
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
    pool: Option<Pool<Postgres>>,
    namespace: String,
}

impl PostgresIdempotencyRepository {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            pool: None,
            namespace: namespace.into(),
        }
    }

    pub fn new_with_pool(pool: Pool<Postgres>, namespace: impl Into<String>) -> Self {
        Self {
            pool: Some(pool),
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
        if let Some(tx_ref) = tx {
            let sqlx_tx = tx_ref.downcast_mut_sqlx()?;

            let row: (bool,) = sqlx::query_as(EXISTS_QUERY)
                .bind(command_id)
                .bind(&self.namespace)
                .fetch_one(&mut **sqlx_tx)
                .await
                .map_err(|e| Error::database(format!("Idempotency check failed (tx): {}", e)))?;
            Ok(row.0)
        } else if let Some(ref pool) = self.pool {
            let row: (bool,) = sqlx::query_as(EXISTS_QUERY)
                .bind(command_id)
                .bind(&self.namespace)
                .fetch_one(pool)
                .await
                .map_err(|e| Error::database(format!("Idempotency check failed (pool): {}", e)))?;
            Ok(row.0)
        } else {
            Err(Error::internal(
                "PostgresIdempotencyRepository: No transaction nor pool available",
            ))
        }
    }

    async fn save(&self, tx: Option<&mut (dyn Transaction + '_)>, command_id: &Uuid) -> Result<()> {
        if let Some(tx_ref) = tx {
            let sqlx_tx = tx_ref.downcast_mut_sqlx()?;

            sqlx::query(SAVE_QUERY)
                .bind(command_id)
                .bind(&self.namespace)
                .execute(&mut **sqlx_tx)
                .await
                .map_err(|e| Error::database(format!("Idempotency save failed (tx): {}", e)))?;
            Ok(())
        } else if let Some(ref pool) = self.pool {
            sqlx::query(SAVE_QUERY)
                .bind(command_id)
                .bind(&self.namespace)
                .execute(pool)
                .await
                .map_err(|e| Error::database(format!("Idempotency save failed (pool): {}", e)))?;
            Ok(())
        } else {
            Err(Error::internal(
                "PostgresIdempotencyRepository: No transaction nor pool available",
            ))
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
