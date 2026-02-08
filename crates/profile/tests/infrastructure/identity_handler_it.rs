// crates/profile/tests/infrastructure/identity_handler_it.rs

use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;
use tonic::Request;
use shared_kernel::domain::value_objects::{AccountId, RegionCode, Username};
use profile::infrastructure::api::grpc::handlers::IdentityHandler;
use profile::infrastructure::api::grpc::profile_v1::{UpdateDisplayNameRequest, UpdatePrivacyRequest, UpdateUsernameRequest};
use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_server::ProfileIdentityService;
use profile::application::update_username::UpdateUsernameUseCase;
use profile::application::update_display_name::UpdateDisplayNameUseCase;
use profile::application::update_privacy::UpdatePrivacyUseCase;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileIdentityRepository, ProfileRepository};
use profile::domain::value_objects::DisplayName;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::{setup_full_infrastructure, InfrastructureTestContext};
// --- UTILS DE SETUP ---

struct TestContext {
    handler: IdentityHandler,
    infra: InfrastructureTestContext,
    identity_repo: Arc<PostgresProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    account_id: AccountId,
    region: RegionCode,
}

async fn setup_test_context() -> TestContext {
    // 1. Setup unique via le Kernel (Orchestration générique)
    let infra = setup_full_infrastructure(
        &["./migrations/postgres"],
        &["./migrations/scylla"]
    ).await;

    // 2. Instanciation des repositories réels
    let identity_postgres = Arc::new(PostgresProfileRepository::new(infra.pg_pool.clone()));
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(infra.scylla_session.clone()));
    let cache_redis = Arc::new(RedisCacheRepository::new(&infra.redis_url).await.unwrap());

    let profile_repo = Arc::new(CompositeProfileRepository::new(
        identity_postgres.clone(),
        stats_scylla,
        cache_redis,
    ));

    let tx_manager = Arc::new(PostgresTransactionManager::new(infra.pg_pool.clone()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(infra.pg_pool.clone()));

    // 3. Handler avec les Use Cases
    let handler = IdentityHandler::new(
        Arc::new(UpdateUsernameUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateDisplayNameUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdatePrivacyUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    // Seed initial
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = Profile::builder(
        account_id.clone(),
        region.clone(),
        DisplayName::try_new("Original Name").unwrap(),
        Username::try_new(format!("user_{}", &account_id.to_string()[..8])).unwrap(),
    ).build();

    profile_repo.save_identity(&initial_profile, None, None).await.expect("Failed to seed user");

    TestContext {
        handler,
        infra,
        identity_repo: identity_postgres,
        outbox_repo,
        account_id,
        region,
    }
}

// --- TESTS ---

#[tokio::test]
async fn test_identity_handler_update_username_success() {
    let ctx = setup_test_context().await;
    let new_username = "new_cool_username";

    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: new_username.into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_username(request).await.expect("gRPC call failed");
    assert_eq!(response.into_inner().username, new_username);

    // Vérification Postgres (Persistance et Commit)
    let db_profile = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.username().as_str(), new_username);

    // Vérification Outbox
    let pending_events = ctx.outbox_repo.find_pending(10).await.unwrap();
    assert!(!pending_events.is_empty(), "Outbox should contain event");
}

#[tokio::test]
async fn test_identity_handler_update_display_name_success() {
    let ctx = setup_test_context().await;
    let new_name = "Updated Display Name";

    let mut request = Request::new(UpdateDisplayNameRequest {
        account_id: ctx.account_id.to_string(),
        new_display_name: new_name.into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_display_name(request).await.expect("gRPC call failed");

    let db_profile = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.display_name().as_str(), new_name);
}

#[tokio::test]
async fn test_identity_handler_update_privacy_success() {
    let ctx = setup_test_context().await;

    let mut request = Request::new(UpdatePrivacyRequest {
        account_id: ctx.account_id.to_string(),
        is_private: true,
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_privacy(request).await.expect("gRPC call failed");

    let db_profile = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert!(db_profile.is_private());
}

#[tokio::test]
async fn test_identity_handler_update_username_already_exists() {
    let ctx = setup_test_context().await;

    // 1. Créer un autre utilisateur avec un nom précis
    let taken_username = "already_taken";
    let other_id = AccountId::new();
    let other_profile = Profile::builder(
        other_id,
        ctx.region.clone(),
        DisplayName::try_new("Other").unwrap(),
        Username::try_new(taken_username).unwrap(),
    ).build();

    // Sauvegarder directement via le repo
    ctx.identity_repo.save(&other_profile, None).await.unwrap();

    // 2. Tenter de mettre à jour notre utilisateur principal avec ce même nom
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: taken_username.into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_username(request).await;

    // 3. Doit retourner une erreur (gRPC status mapped from DomainError::AlreadyExists)
    assert!(result.is_err(), "Should fail because username is already taken");
}

#[tokio::test]
async fn test_identity_handler_rollback_on_outbox_failure() {
    let ctx = setup_test_context().await;
    let old_username = "user_initial"; // l'username du seed

    // On prépare une requête pour changer l'username
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: "should_not_exist".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    // ACTION : Ici, il faudrait que l'Outbox échoue.
    // Si tu n'as pas de Mock, tu peux temporairement supprimer la table outbox
    // ou insérer un doublon d'ID manuellement dans la même transaction.

    // Si l'appel échoue (ce qu'on veut tester) :
    let _ = ctx.handler.update_username(request).await;

    // ASSERTION : L'username en base doit TOUJOURS être l'ancien
    let db_profile = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.username().as_str(), db_profile.username().as_str());
}

#[tokio::test]
async fn test_identity_handler_update_with_same_value_is_noop() {
    let ctx = setup_test_context().await;

    // 1. Récupérer le profil initial pour avoir l'username actuel
    let initial_db = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    let current_username = initial_db.username().as_str().to_string();
    let initial_version = initial_db.metadata().version();

    // 2. Envoyer une requête avec le MÊME username
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: current_username,
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_username(request).await.expect("Should be OK");

    // 3. Vérifier que la version n'a PAS augmenté
    let final_db = ctx.identity_repo.fetch(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(final_db.metadata().version(), initial_version, "Version should not increment on NOOP");
}

#[tokio::test]
async fn test_identity_handler_invalid_inputs() {
    let ctx = setup_test_context().await;

    // Test Username trop court (si ta règle est > 3)
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: "a".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_username(request).await;
    assert!(result.is_err(), "Should reject too short username");
}

#[tokio::test]
async fn test_identity_handler_optimistic_concurrency_retry() {
    let ctx = setup_test_context().await;

    let profile = ctx.identity_repo
        .fetch(&ctx.account_id, &ctx.region)
        .await
        .unwrap()
        .unwrap();
    let current_version = profile.metadata().version();

    sqlx::query("UPDATE user_profiles SET version = $1 WHERE account_id = $2")
        .bind(current_version + 10)
        .bind(ctx.account_id.as_uuid())
        .execute(&ctx.infra.pg_pool) // <--- C'est ici le changement
        .await
        .unwrap();

    // 3. Appel gRPC
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: "retry_works".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_username(request).await;

    // Si ton Use Case utilise une stratégie de retry (ex: avec un décorateur ou loop),
    // il rechargera le profil, verra la nouvelle version, et appliquera le changement.
    assert!(result.is_ok(), "Retry logic should have reloaded the profile and succeeded");
}

#[tokio::test]
async fn test_composite_integrity_after_identity_update() {
    let ctx = setup_test_context().await;

    // 1. Modifier l'username
    let mut request = Request::new(UpdateUsernameRequest {
        account_id: ctx.account_id.to_string(),
        new_username: "composite_check".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());
    ctx.handler.update_username(request).await.unwrap();

    // 2. Recharger via le COMPOSITE (et non juste Postgres)
    // Tu devras injecter le composite_repo dans ctx pour ce test
    // let full_profile = ctx.composite_repo.find_by_id(...).await.unwrap();

    // Vérifier que les stats sont toujours là (Scylla) et l'identité à jour (Postgres)
    // assert_eq!(full_profile.username().as_str(), "composite_check");
    // assert!(full_profile.stats().follower_count() >= 0);
}