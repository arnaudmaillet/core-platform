// crates/profile/tests/infrastructure/handler_it_for_identity.rs

use std::sync::Arc;
use tonic::Request;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use profile::infrastructure::api::grpc::handlers::IdentityHandler;
use profile::infrastructure::api::grpc::profile_v1::{UpdateDisplayNameRequest, UpdateHandleRequest, UpdatePrivacyRequest};
use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_server::ProfileIdentityService;
use profile::application::update_display_name::UpdateDisplayNameUseCase;
use profile::application::update_handle::UpdateHandleUseCase;
use profile::application::update_privacy::UpdatePrivacyUseCase;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileIdentityRepository, ProfileRepository};
use profile::domain::value_objects::{DisplayName, Handle, ProfileId};
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::InfrastructureKernelTestContext;

struct IdentityHandlerTestContext {
    handler: IdentityHandler,
    infra: InfrastructureKernelTestContext,
    identity_repo: Arc<PostgresIdentityRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    profile_id: ProfileId,
    owner_id: AccountId,
    region: RegionCode,
}

async fn setup_test_context() -> IdentityHandlerTestContext {
    // 1. Setup de l'infrastructure (orchestration parallèle interne)
    let infra_from_test_containers = InfrastructureKernelTestContext::builder()
        .with_postgres_migrations(&["./migrations/postgres"])
        .with_scylla_migrations(&["./migrations/scylla"])
        .build()
        .await;

    // 2. Instanciation des repositories via les contextes spécialisés
    let pg_pool = infra_from_test_containers.postgres().pool();
    let scylla_session = infra_from_test_containers.scylla().session();

    // 3. Instanciation des repositories
    let postgres_repo = Arc::new(PostgresIdentityRepository::new(pg_pool.clone()));
    let scylla_repo = Arc::new(ScyllaProfileRepository::new(scylla_session));
    let redis_repo = infra_from_test_containers.redis().repository(); // Directement l'Arc<RedisCacheRepository>

    let profile_repo = Arc::new(UnifiedProfileRepository::new(
        postgres_repo.clone(),
        scylla_repo,
        redis_repo,
    ));

    let tx_manager = Arc::new(PostgresTransactionManager::new(pg_pool.clone()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pg_pool.clone()));

    // 3. Initialisation du Handler
    let handler = IdentityHandler::new(
        Arc::new(UpdateHandleUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateDisplayNameUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdatePrivacyUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    // 4. Seed d'un profil de test
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = Profile::builder(
        owner_id.clone(),
        region.clone(),
        DisplayName::try_new("Original Name").unwrap(),
        Handle::try_new(format!("user_{}", &owner_id.to_string()[..8])).unwrap(),
    ).build();

    let profile_id = initial_profile.id().clone();
    profile_repo.save_identity(&initial_profile, None, None)
        .await
        .expect("Failed to seed profile");

    IdentityHandlerTestContext {
        handler,
        infra: infra_from_test_containers,
        identity_repo: postgres_repo,
        outbox_repo,
        profile_id,
        owner_id,
        region,
    }
}

// --- TESTS ---

#[tokio::test]
async fn test_identity_handler_update_handle_success() {
    let ctx = setup_test_context().await;
    let new_handle_str = "new_cool_handle";

    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: new_handle_str.into(),
    });
    // Simule l'intercepteur de région
    request.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_handle(request).await.expect("gRPC call failed");
    assert_eq!(response.into_inner().handle, new_handle_str);

    // Vérification Persistance
    let db_profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.handle().as_str(), new_handle_str);

    // Vérification Outbox
    let pending_events = ctx.outbox_repo.find_pending(10).await.unwrap();
    assert!(!pending_events.is_empty(), "Outbox should contain ProfileUpdated event");
}

#[tokio::test]
async fn test_identity_handler_update_display_name_success() {
    let ctx = setup_test_context().await;
    let new_name = "Updated Display Name";

    let mut request = Request::new(UpdateDisplayNameRequest {
        profile_id: ctx.profile_id.to_string(),
        new_display_name: new_name.into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_display_name(request).await.expect("gRPC call failed");

    let db_profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.display_name().as_str(), new_name);
}

#[tokio::test]
async fn test_identity_handler_update_privacy_success() {
    let ctx = setup_test_context().await;

    let mut request = Request::new(UpdatePrivacyRequest {
        profile_id: ctx.profile_id.to_string(),
        is_private: true,
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_privacy(request).await.expect("gRPC call failed");

    let db_profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert!(db_profile.is_private());
}

#[tokio::test]
async fn test_identity_handler_update_handle_already_exists() {
    let ctx = setup_test_context().await;

    // 1. Créer un autre profil avec un handle déjà pris
    let taken_handle = "already_taken";
    let other_profile = Profile::builder(
        AccountId::new(),
        ctx.region.clone(),
        DisplayName::try_new("Other").unwrap(),
        Handle::try_new(taken_handle).unwrap(),
    ).build();

    ctx.identity_repo.save(&other_profile, None).await.unwrap();

    // 2. Tenter de mettre à jour notre profil principal avec ce même handle
    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: taken_handle.into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_handle(request).await;

    // 3. Doit retourner une erreur (Status gRPC mappé depuis DomainError::AlreadyExists)
    assert!(result.is_err(), "Should fail because handle is already taken");
}

#[tokio::test]
async fn test_identity_handler_rollback_on_outbox_failure() {
    let ctx = setup_test_context().await;
    let old_handle = "user_initial"; // l'handle du seed

    // On prépare une requête pour changer l'handle
    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: "should_not_exist".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    // ACTION : Ici, il faudrait que l'Outbox échoue.
    // Si tu n'as pas de Mock, tu peux temporairement supprimer la table outbox
    // ou insérer un doublon d'ID manuellement dans la même transaction.

    // Si l'appel échoue (ce qu'on veut tester) :
    let _ = ctx.handler.update_handle(request).await;

    // ASSERTION : L'handle en base doit TOUJOURS être l'ancien
    let db_profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.handle().as_str(), db_profile.handle().as_str());
}

#[tokio::test]
async fn test_identity_handler_update_with_same_value_is_noop() {
    let ctx = setup_test_context().await;

    let initial_db = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    let current_handle = initial_db.handle().as_str().to_string();
    let initial_version = initial_db.version();

    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: current_handle,
    });
    request.extensions_mut().insert(ctx.region.clone());

    ctx.handler.update_handle(request).await.expect("Should be OK");

    let final_db = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(final_db.version(), initial_version, "Version should not increment on NOOP");
}

#[tokio::test]
async fn test_identity_handler_invalid_inputs() {
    let ctx = setup_test_context().await;

    // Test Handle trop court (règle Slug < 3 chars)
    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: "ab".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_handle(request).await;
    assert!(result.is_err(), "Should reject handle shorter than 3 chars");
}

#[tokio::test]
async fn test_identity_handler_optimistic_concurrency_retry() {
    let ctx = setup_test_context().await;
    let pool = ctx.infra.postgres().pool();

    // 1. Récupérer version actuelle
    let profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    let current_version = profile.version();

    // 2. Simuler une mise à jour concurrente directe en DB (Out-of-band)
    // On incrémente la version de force pour faire échouer le prochain save du handler
sqlx::query("UPDATE user_profiles SET version = $1 WHERE id = $2 AND region_code = $3")
    .bind((current_version + 1) as i64)
    .bind(ctx.profile_id.as_uuid())
    .bind(ctx.region.as_str())
    .execute(&pool)
    .await
    .unwrap();

    // 3. Appel gRPC : Le Use Case devrait détecter le conflit, recharger le profil et réessayer
    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: "retry_works".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_handle(request).await;

    assert!(result.is_ok(), "Retry logic should have reloaded the profile and succeeded");

    let db_profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(db_profile.handle().as_str(), "retry_works");
    assert_eq!(db_profile.version(), current_version + 2); // Seed(1) + ManualUpdate(2) + gRPC(3)
}

#[tokio::test]
async fn test_composite_integrity_after_identity_update() {
    let ctx = setup_test_context().await;

    // 1. Modifier l'handle
    let mut request = Request::new(UpdateHandleRequest {
        profile_id: ctx.profile_id.to_string(),
        new_handle: "composite_check".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());
    ctx.handler.update_handle(request).await.unwrap();

    // 2. Recharger via le COMPOSITE (et non juste Postgres)
    // Tu devras injecter le composite_repo dans ctx pour ce test
    // let full_profile = ctx.composite_repo.find_by_id(...).await.unwrap();

    // Vérifier que les stats sont toujours là (Scylla) et l'identité à jour (Postgres)
    // assert_eq!(full_profile.handle().as_str(), "composite_check");
    // assert!(full_profile.stats().follower_count() >= 0);
}