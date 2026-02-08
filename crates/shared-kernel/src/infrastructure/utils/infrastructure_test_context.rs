// crates/shared-kernel/src/infrastructure/utils/infrastructure_test_context.rs
#![cfg(feature = "test-utils")]

use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres as PostgresImage;
use testcontainers_modules::redis::Redis as RedisImage;
use scylla::client::session::Session as ScyllaSession;

use crate::infrastructure::postgres::utils::setup_test_postgres;
use crate::infrastructure::redis::utils::setup_test_redis;
use crate::infrastructure::scylla::utils::setup_test_scylla;

pub struct InfrastructureTestContext {
    pub pg_pool: sqlx::PgPool,
    pub redis_url: String,
    pub scylla_session: Arc<ScyllaSession>,
    // On cache les containers pour garantir qu'ils vivent
    // aussi longtemps que la structure de contexte.
    _pg_container: ContainerAsync<PostgresImage>,
    _redis_container: ContainerAsync<RedisImage>,
    // Scylla est géré par ton Singleton, donc pas besoin de le stocker ici
}

pub async fn setup_full_infrastructure(
    pg_migrations: &[&str],
    scylla_migrations: &[&str]
) -> InfrastructureTestContext {
    // On lance tout en parallèle
    let (pg_res, redis_res, scylla_res) = tokio::join!(
        setup_test_postgres(pg_migrations),
        setup_test_redis(),
        setup_test_scylla(scylla_migrations)
    );

    let (pg_pool, pg_container) = pg_res;
    let (redis_url, redis_container) = redis_res;
    let scylla_session = scylla_res;

    InfrastructureTestContext {
        pg_pool,
        redis_url,
        scylla_session,
        _pg_container: pg_container,
        _redis_container: redis_container,
    }
}