// crates/profile/tests/common/setup_infrastructure.rs

use std::sync::Arc;
use scylla::client::session::Session;
use sqlx::PgPool;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use testcontainers_modules::redis::Redis as RedisImage;
use shared_kernel::infrastructure::postgres::utils::setup_test_postgres;
use shared_kernel::infrastructure::redis::utils::setup_test_redis;
use shared_kernel::infrastructure::scylla::utils::setup_test_scylla;

pub async fn setup_postgres_test_db() -> (PgPool, ContainerAsync<PostgresImage>) {
    let profile_migrations = "crates/profile/migrations/postgres";
    let path = if std::path::Path::new(profile_migrations).exists() {
        profile_migrations
    } else {
        "./migrations/postgres"
    };

    setup_test_postgres(&[path]).await
}

pub async fn setup_scylla_db() -> Arc<Session> {
    let migration_path = if std::env::var("BAZEL_TEST").is_ok() {
        "crates/profile/migrations/scylla"
    } else {
        "./migrations/scylla"
    };

    setup_test_scylla(&[migration_path]).await
}

pub async fn setup_redis_test_cache() -> (String, ContainerAsync<RedisImage>) {
    setup_test_redis().await
}

