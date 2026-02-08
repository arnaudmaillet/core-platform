// crates/profile/tests/infrastructure/account_consumer_it.rs

use std::sync::Arc;
use uuid::Uuid;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;
use profile::application::create_profile::CreateProfileUseCase;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::infrastructure::kafka::AccountConsumer;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;

struct ConsumerTestContext {
    consumer: AccountConsumer,
    profile_repo: Arc<PostgresProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    pool: sqlx::PgPool,
    _pg_container: ContainerAsync<Postgres>,
    _redis_container: ContainerAsync<Redis>,
}

async fn setup_consumer_test_context() -> ConsumerTestContext {
    // 1. Démarrage parallèle des dépendances
    let (pg_setup, redis_setup) = tokio::join!(
        crate::common::setup_postgres_test_db(),
        crate::common::setup_redis_test_cache()
    );

    let (pool, pg_container) = pg_setup;
    let (redis_url, redis_container) = redis_setup;

    // 2. Instanciation des composants techniques
    let identity_postgres = Arc::new(PostgresProfileRepository::new(pool.clone()));

    // Pour Scylla, on utilise ton helper common existant
    let scylla_session = crate::common::setup_scylla_db().await;
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(scylla_session));

    // Nouveau : Le cache Redis réel
    let cache_redis = Arc::new(RedisCacheRepository::new(&redis_url).await.unwrap());

    // 3. Le Composite qui orchestre le tout
    let profile_repo = Arc::new(CompositeProfileRepository::new(
        identity_postgres.clone(),
        stats_scylla.clone(),
        cache_redis,
    ));

    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));
    let tx_manager = Arc::new(PostgresTransactionManager::new(pool.clone()));

    let use_case = Arc::new(CreateProfileUseCase::new(
        profile_repo,
        outbox_repo.clone(),
        tx_manager,
    ));

    let consumer = AccountConsumer::new(use_case);

    ConsumerTestContext {
        consumer,
        profile_repo: identity_postgres,
        outbox_repo,
        pool,
        _pg_container: pg_container,
        _redis_container: redis_container,
    }
}

#[tokio::test]
async fn test_consumer_creates_profile_on_account_created_event() {
    let ctx = setup_consumer_test_context().await;
    let account_id = Uuid::now_v7();
    let region = "eu";

    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": account_id,
            "region": region,
            "username": "tester_66",
            "display_name": "Tester Sixty Six",
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    ctx.consumer.on_message_received(&bytes).await.expect("Should work");

    // Note : On utilise fetch_identity_by_id car on a renommé nos méthodes de repo
    let profile = ctx.profile_repo
        .fetch(&AccountId::from(account_id), &RegionCode::try_new(region).unwrap())
        .await
        .unwrap()
        .expect("Profile should exist in database");

    assert_eq!(profile.username().as_str(), "tester_66");
}

#[tokio::test]
async fn test_consumer_fallback_on_invalid_display_name() {
    let ctx = setup_consumer_test_context().await;
    let account_id = Uuid::now_v7();

    // On envoie un display_name vide (qui serait rejeté par le VO DisplayName)
    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": account_id,
            "region": "us",
            "username": "safe_username",
            "display_name": "",
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    ctx.consumer.on_message_received(&bytes).await.unwrap();

    let profile = ctx.profile_repo
        .fetch(&AccountId::from(account_id), &RegionCode::try_new("us").unwrap())
        .await
        .unwrap()
        .unwrap();

    // On vérifie que le fallback a fonctionné : le display_name est devenu le username
    assert_eq!(profile.display_name().as_str(), "safe_username");
}

#[tokio::test]
async fn test_consumer_is_idempotent() {
    let ctx = setup_consumer_test_context().await;
    let account_id = Uuid::now_v7();
    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": account_id,
            "region": "eu",
            "username": "unique_tester",
            "display_name": "Unique",
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    // Premier passage
    ctx.consumer.on_message_received(&bytes).await.expect("First pass should work");

    // Deuxième passage (simule un retry Kafka)
    let result = ctx.consumer.on_message_received(&bytes).await;

    // Doit être OK (soit ignoré, soit géré par un ON CONFLICT)
    assert!(result.is_ok(), "Second pass should not return an error (idempotency)");
}

#[tokio::test]
async fn test_consumer_ignores_unknown_event_types() {
    let ctx = setup_consumer_test_context().await;

    let payload = serde_json::json!({
        "type": "account.password_changed", // Event inconnu pour le module Profile
        "data": { "account_id": Uuid::now_v7() }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    let result = ctx.consumer.on_message_received(&bytes).await;
    assert!(result.is_ok(), "Should silently ignore unknown event types");
}

#[tokio::test]
async fn test_consumer_fails_on_corrupted_json() {
    let ctx = setup_consumer_test_context().await;
    let corrupted_bytes = b"{ invalid json ]";

    let result = ctx.consumer.on_message_received(corrupted_bytes).await;
    assert!(result.is_err(), "Should return error on corrupted payload");
}