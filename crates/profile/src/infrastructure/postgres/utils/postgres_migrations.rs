// crates/profile/src/infrastructure/postgres/utils/postgres_migrations.rs

pub async fn run_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations/postgres")
        .run(pool)
        .await?;
    Ok(())
}