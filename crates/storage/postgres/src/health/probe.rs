//! Ready-made [`HealthProbe`] over a Postgres pool, so services don't hand-roll
//! the readiness closure.

use std::sync::Arc;

use async_trait::async_trait;
use health::HealthProbe;
use sqlx::PgPool;

struct PostgresHealthProbe {
    pool: PgPool,
}

#[async_trait]
impl HealthProbe for PostgresHealthProbe {
    fn name(&self) -> &str {
        "postgres"
    }

    async fn check(&self) -> anyhow::Result<()> {
        super::check::health_check(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("postgres: {e}"))
    }
}

/// Builds a readiness probe (name `"postgres"`) over a live Postgres pool.
pub fn probe(pool: PgPool) -> Arc<dyn HealthProbe> {
    Arc::new(PostgresHealthProbe { pool })
}
