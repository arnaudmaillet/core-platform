// crates/shared-kernel/src/infrastructure/postgres/utils/postgres_migrations.rs

pub async fn run_kernel_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations/postgres").run(pool).await?;
    println!("✅ Profile domain migrations applied");

    Ok(())
}
