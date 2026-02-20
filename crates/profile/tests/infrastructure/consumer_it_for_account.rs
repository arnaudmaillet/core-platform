// crates/profile/tests/infrastructure/consumer_it_for_account.rs

use std::sync::Arc;
use uuid::Uuid;
use profile::application::use_cases::create_profile::CreateProfileUseCase;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::infrastructure::kafka::AccountConsumer;
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::value_objects::AccountId;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::utils::InfrastructureKernelTestContext;

struct AccountConsumerTestContext {
    consumer: AccountConsumer,
    profile_repo: Arc<PostgresIdentityRepository>,
    _infra: InfrastructureKernelTestContext,
}

async fn setup_consumer_test_context() -> AccountConsumerTestContext {
    // 1. On configure et on lance tout ici, c'est très explicite
    let infra_from_test_containers = InfrastructureKernelTestContext::builder()
        .with_postgres_migrations(&["./migrations/postgres"])
        .with_scylla_migrations(&["./migrations/scylla"])
        .build()
        .await;

    // 2. On instancie les repositories (Note les chemins d'accès via _ctx)
    let pg_pool = infra_from_test_containers.postgres().pool();
    let scylla_session = infra_from_test_containers.scylla().session();

    let postgres_repo = Arc::new(PostgresIdentityRepository::new(pg_pool.clone()));
    let scylla_repo = Arc::new(ScyllaProfileRepository::new(scylla_session));
    let redis_repo = infra_from_test_containers.redis().repository();

    let profile_repo = Arc::new(UnifiedProfileRepository::new(
        postgres_repo.clone(),
        scylla_repo,
        redis_repo,
    ));

    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pg_pool.clone()));
    let tx_manager = Arc::new(PostgresTransactionManager::new(pg_pool));

    let use_case = Arc::new(CreateProfileUseCase::new(
        profile_repo,
        outbox_repo,
        tx_manager,
    ));

    let consumer = AccountConsumer::new(use_case);

    AccountConsumerTestContext {
        consumer,
        profile_repo: postgres_repo,
        _infra: infra_from_test_containers,
    }
}

#[tokio::test]
async fn test_consumer_creates_profile_on_account_created_event() {
    let ctx = setup_consumer_test_context().await;
    let owner_id = Uuid::now_v7();
    let region = "eu";

    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": owner_id,
            "region": region,
            "username": "tester_66",
            "display_name": "Tester Sixty Six",
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    // Act
    ctx.consumer.on_message_received(&bytes).await.expect("Should work");

    // Assert
    let profiles = ctx.profile_repo
        .fetch_all_by_owner(&AccountId::from(owner_id))
        .await
        .unwrap();

    assert_eq!(profiles.len(), 1, "Un profil aurait dû être créé");
    let profile = &profiles[0];

    assert_eq!(profile.handle().as_str(), "tester_66");
    assert_eq!(profile.region_code().as_str(), "eu");
}

#[tokio::test]
async fn test_consumer_fallback_on_invalid_display_name() {
    let ctx = setup_consumer_test_context().await;
    let owner_id = Uuid::now_v7();

    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": owner_id, // FIX: account_id et non owner_id
            "region": "us",
            "username": "safe_handle",
            "display_name": "", // Devrait trigger le fallback sur le username
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    ctx.consumer.on_message_received(&bytes).await.expect("Message processing failed");

    let profiles = ctx.profile_repo
        .fetch_all_by_owner(&AccountId::from(owner_id))
        .await
        .unwrap();

    let profile = profiles.first().expect("Profile should exist");

    // Fallback : si display_name est vide, le consumer doit utiliser le handle
    assert_eq!(profile.display_name().as_str(), "safe_handle");
}

#[tokio::test]
async fn test_consumer_is_idempotent() {
    let ctx = setup_consumer_test_context().await;
    let owner_id = Uuid::now_v7();

    let payload = serde_json::json!({
        "type": "account.created",
        "data": {
            "account_id": owner_id, // FIX: account_id et non owner_id
            "region": "eu",
            "username": "unique_tester",
            "display_name": "Unique",
            "occurred_at": "2026-02-04T12:00:00Z"
        }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    // Premier passage : Création
    ctx.consumer.on_message_received(&bytes).await.expect("First pass should work");

    // Deuxième passage : Doit ignorer (grâce au catch AlreadyExists dans le consumer)
    let result = ctx.consumer.on_message_received(&bytes).await;

    assert!(result.is_ok(), "Second pass should not return an error (idempotency)");

    let profiles = ctx.profile_repo
        .fetch_all_by_owner(&AccountId::from(owner_id))
        .await
        .unwrap();

    assert_eq!(profiles.len(), 1, "Il ne doit toujours y avoir qu'un seul profil après le replay");
}

#[tokio::test]
async fn test_consumer_ignores_unknown_event_types() {
    let ctx = setup_consumer_test_context().await;

    let payload = serde_json::json!({
        "type": "account.email_verified",
        "data": { "account_id": Uuid::now_v7() }
    });
    let bytes = serde_json::to_vec(&payload).unwrap();

    let result = ctx.consumer.on_message_received(&bytes).await;
    assert!(result.is_ok(), "Should silently ignore unknown event types thanks to #[serde(other)]");
}

#[tokio::test]
async fn test_consumer_fails_on_corrupted_json() {
    let ctx = setup_consumer_test_context().await;
    // JSON invalide (manque de guillemets, etc)
    let corrupted_bytes = b"{ \"type\": \"account.created\", \"data\": corrupted }";

    let result = ctx.consumer.on_message_received(corrupted_bytes).await;
    assert!(result.is_err(), "Should return error on corrupted payload for monitoring/dead-letter purposes");
}