// crates/shared-kernel/src/infrastructure/postgres/repositories/postgres_idempotency_repository.rs

use crate::domain::repositories::IdempotencyRepository;
use crate::domain::transaction::Transaction;
use crate::errors::Result;
use crate::infrastructure::postgres::mappers::SqlxErrorExt;
use crate::infrastructure::postgres::transactions::TransactionExt;
use async_trait::async_trait;
use uuid::Uuid;

pub struct PostgresIdempotencyRepository {
    namespace: String,
}

impl PostgresIdempotencyRepository {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self { 
            namespace: namespace.into() 
        }
    }
}

#[async_trait]
impl IdempotencyRepository for PostgresIdempotencyRepository {
    async fn exists(&self, tx: &mut dyn Transaction, command_id: &Uuid) -> Result<bool> {
        let sqlx_tx = tx.downcast_mut_sqlx()?;
        
        let sql = "SELECT EXISTS(SELECT 1 FROM processed_commands WHERE command_id = $1 AND namespace = $2)";
        
        let row: (bool,) = sqlx::query_as(sql)
            .bind(command_id)
            .bind(&self.namespace)
            .fetch_one(&mut **sqlx_tx)
            .await
            .map_domain_infra("Idempotency Check")?;

        Ok(row.0)
    }

    async fn save(&self, tx: &mut dyn Transaction, command_id: &Uuid) -> Result<()> {
        let sqlx_tx = tx.downcast_mut_sqlx()?;
        let sql = r#"
            INSERT INTO processed_commands (command_id, namespace, processed_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (command_id, namespace) DO NOTHING
        "#;

        sqlx::query(sql)
            .bind(command_id)
            .bind(&self.namespace)
            .execute(&mut **sqlx_tx)
            .await
            .map_domain_infra("Idempotency Save")?;

        Ok(())
    }
}