// crates/profile/tests/infrastructure/account_consumer_it.rs

use std::sync::Arc;
use uuid::Uuid;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use profile::application::create_profile::CreateProfileUseCase;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::infrastructure::kafka::AccountConsumer;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;

struct ConsumerTestContext {
    consumer: AccountConsumer,
    profile_repo: Arc<PostgresProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    pool: sqlx::PgPool,
    _pg_container: ContainerAsync<Postgres>,
}

async fn setup_consumer_test_context() -> ConsumerTestContext {
    let (pool, pg_container) = crate::common::setup_postgres_test_db().await;

    // 1. On a besoin des deux briques pour le Composite
    let identity_postgres = Arc::new(PostgresProfileRepository::new(pool.clone()));

    // Si tu as une session Scylla de test dispo :
    let scylla_session = crate::common::setup_scylla_db().await;
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(scylla_session));

    // 2. On crée le Composite qui, LUI, implémente ProfileRepository
    let profile_repo = Arc::new(CompositeProfileRepository::new(
        identity_postgres.clone(),
        stats_scylla.clone(),
    ));

    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));
    let tx_manager = Arc::new(PostgresTransactionManager::new(pool.clone()));

    // 3. Maintenant le cast Arc<CompositeProfileRepository> -> Arc<dyn ProfileRepository> fonctionne
    let use_case = Arc::new(CreateProfileUseCase::new(
        profile_repo,
        outbox_repo.clone(),
        tx_manager,
    ));

    let consumer = AccountConsumer::new(use_case);

    ConsumerTestContext {
        consumer,
        profile_repo: identity_postgres, // On garde l'accès direct à Postgres pour les assertions
        outbox_repo,
        pool,
        _pg_container: pg_container,
    }
}
#[tokio::test]
async fn test_consumer_creates_profile_on_account_created_event() {
    let ctx = setup_consumer_test_context().await;
    let account_id = Uuid::now_v7();
    let region = "eu";

    // 1. On simule le payload JSON tel qu'il sortirait de l'Outbox du module Account
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

    // 2. Action : Le consumer traite les octets reçus de "Kafka"
    ctx.consumer.on_message_received(&bytes).await.expect("Consumer should process valid message");

    // 3. Assertions : Vérification dans Postgres
    let profile = ctx.profile_repo
        .find_by_id(&AccountId::from(account_id), &RegionCode::try_new(region).unwrap())
        .await
        .unwrap()
        .expect("Profile should exist in database");

    assert_eq!(profile.username().as_str(), "tester_66");
    assert_eq!(profile.display_name().as_str(), "Tester Sixty Six");
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
        .find_by_id(&AccountId::from(account_id), &RegionCode::try_new("us").unwrap())
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