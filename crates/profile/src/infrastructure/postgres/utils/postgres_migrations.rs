// crates/profile/src/infrastructure/postgres/utils/postgres_migrations.rs

use shared_kernel::infrastructure::postgres::utils::run_kernel_postgres_migrations;

pub async fn run_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // 1. On s'assure d'abord que le socle commun est prêt
    run_kernel_postgres_migrations(pool).await?;
    println!("✅ Shared Kernel migrations applied");

    // 2. On applique les migrations spécifiques au domaine Profile
    sqlx::migrate!("./migrations/postgres").run(pool).await?;
    println!("✅ Profile domain migrations applied");

    Ok(())
}
