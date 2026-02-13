// crates/profile/src/infrastructure/postgres/utils/postgres_migrations.rs

use sqlx::Executor;
use shared_kernel::infrastructure::postgres::utils::run_kernel_postgres_migrations;

pub async fn run_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // 1. On s'assure d'abord que le socle commun est prêt
    run_kernel_postgres_migrations(pool).await?;

    // 2. On applique les migrations spécifiques au domaine Profile
    let schema = include_str!("../../../../migrations/postgres/202601030000_profile.sql");
    pool.execute(schema).await?;
    println!("✅ Profile domain migrations applied (via include_str)");
    Ok(())
}
