// crates/profile/tests/common/setup_test_db.rs

use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use shared_kernel::infrastructure::postgres::utils::setup_test_postgres;

pub async fn setup_postgres_test_db() -> (PgPool, ContainerAsync<PostgresImage>) {
    setup_test_postgres(&[
        "./migrations/postgres"
    ]).await
}