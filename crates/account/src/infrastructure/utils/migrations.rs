// crates/account/src/infrastructure/utils/migrations.rs

use shared_kernel::postgres::run_kernel_postgres_migrations;
use sqlx::Executor;

pub async fn run_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // 1. On s'assure d'abord que le socle commun est prêt
    run_kernel_postgres_migrations(pool).await?;

    // 2. On applique les migrations spécifiques au domaine Account
    let schema = include_str!("../../../migrations/postgres/202601020000_account.sql");
    pool.execute(schema).await?;
    tracing::info!("Account domain migrations successfully applied via include_str");
    Ok(())
}
