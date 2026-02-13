// crates/shared-kernel/src/infrastructure/postgres/utils/postgres_migrations.rs

pub async fn run_kernel_postgres_migrations(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // Cette macro native Rust est comprise par Bazel et Cargo
    let schema = include_str!("../../../../migrations/postgres/202601010000_foundation.sql");
    sqlx::query(schema).execute(pool).await?;
    println!("✅ Shared Kernel migrations applied (via include_str)");
    Ok(())
}
