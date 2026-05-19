// crates/shared-kernel/src/persistence/postgres/utils/migrations.rs

use sqlx::Executor;

pub async fn run_kernel_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let schema = include_str!("../../../../migrations/postgres/202601010000_foundation.sql");
    pool.execute(schema).await?;
    tracing::info!("Shared Kernel migrations successfully applied via include_str");
    Ok(())
}
