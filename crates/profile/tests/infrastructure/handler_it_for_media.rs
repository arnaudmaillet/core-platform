// crates/profile/tests/infrastructure/handler_it_for_media.rs

use std::sync::Arc;
use tonic::Request;

use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use profile::infrastructure::api::grpc::handlers::MediaHandler;
use profile::infrastructure::api::grpc::profile_v1::{
    UpdateAvatarRequest, RemoveAvatarRequest, UpdateBannerRequest, RemoveBannerRequest
};
use profile::infrastructure::api::grpc::profile_v1::profile_media_service_server::ProfileMediaService;

use profile::application::update_avatar::UpdateAvatarUseCase;
use profile::application::remove_avatar::RemoveAvatarUseCase;
use profile::application::update_banner::UpdateBannerUseCase;
use profile::application::remove_banner::RemoveBannerUseCase;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileIdentityRepository, ProfileRepository};
use profile::domain::value_objects::{DisplayName, Handle, ProfileId};
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::InfrastructureKernelTestContext;

// --- UTILS DE SETUP ---

struct MediaHandlerTestContext {
    handler: MediaHandler,
    infra: InfrastructureKernelTestContext,
    identity_repo: Arc<PostgresIdentityRepository>,
    composite_repo: Arc<UnifiedProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    profile_id: ProfileId,
    region: RegionCode,
}

async fn setup_test_context() -> MediaHandlerTestContext {
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

    // 3. Initialisation du MediaHandler
    let handler = MediaHandler::new(
        Arc::new(UpdateAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(RemoveAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(RemoveBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    // 4. Seed initial
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = Profile::builder(
        owner_id,
        region.clone(),
        DisplayName::try_new("Media User").unwrap(),
        Handle::try_new("media_pro").unwrap(),
    ).build();

    let profile_id = initial_profile.id().clone();
    profile_repo.save_identity(&initial_profile, None, None)
        .await
        .expect("Failed to seed profile");

    MediaHandlerTestContext {
        handler,
        infra: infra_from_test_containers,
        identity_repo: postgres_repo,
        composite_repo: profile_repo,
        outbox_repo,
        profile_id,
        region,
    }
}

// --- TESTS ---

#[tokio::test]
async fn test_media_handler_avatar_lifecycle() {
    let ctx = setup_test_context().await;
    let avatar_url = "https://cdn.assets.com/avatars/123.png";

    // 1. UPDATE AVATAR
    let mut update_req = Request::new(UpdateAvatarRequest {
        profile_id: ctx.profile_id.to_string(),
        new_avatar_url: avatar_url.into(),
    });
    update_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_avatar(update_req).await.expect("Update avatar failed");
    assert_eq!(response.into_inner().avatar_url, Some(avatar_url.into()));

    // Vérification DB (Fetch par ProfileId)
    let profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(profile.avatar_url().unwrap().as_str(), avatar_url);

    // 2. REMOVE AVATAR
    let mut remove_req = Request::new(RemoveAvatarRequest {
        profile_id: ctx.profile_id.to_string(),
    });
    remove_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.remove_avatar(remove_req).await.expect("Remove avatar failed");
    assert_eq!(response.into_inner().avatar_url, None);

    let profile_final = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert!(profile_final.avatar_url().is_none());
}

#[tokio::test]
async fn test_media_handler_banner_lifecycle() {
    let ctx = setup_test_context().await;
    let banner_url = "https://cdn.assets.com/banners/456.jpg";

    // 1. UPDATE BANNER
    let mut update_req = Request::new(UpdateBannerRequest {
        profile_id: ctx.profile_id.to_string(),
        new_banner_url: banner_url.into(),
    });
    update_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_banner(update_req).await.expect("Update banner failed");
    assert_eq!(response.into_inner().banner_url, Some(banner_url.into()));

    // 2. REMOVE BANNER
    let mut remove_req = Request::new(RemoveBannerRequest {
        profile_id: ctx.profile_id.to_string(),
    });
    remove_req.extensions_mut().insert(ctx.region.clone());

    ctx.handler.remove_banner(remove_req).await.expect("Remove banner failed");

    // Vérification Outbox
    let pending = ctx.outbox_repo.find_pending(10).await.unwrap();
    assert!(pending.len() >= 2);
}

#[tokio::test]
async fn test_media_handler_invalid_url_format() {
    let ctx = setup_test_context().await;

    let mut request = Request::new(UpdateAvatarRequest {
        profile_id: ctx.profile_id.to_string(),
        new_avatar_url: "ftp://not-allowed-scheme.com".into(), // Supposons que ton VO Url rejette ftp
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_avatar(request).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_media_handler_profile_not_found() {
    let ctx = setup_test_context().await;
    let random_pid = ProfileId::new(); // ID inexistant

    let mut request = Request::new(RemoveAvatarRequest {
        profile_id: random_pid.to_string(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.remove_avatar(request).await;

    assert!(result.is_err());
    // On s'attend à un NotFound mappé depuis DomainError::NotFound
    assert_eq!(result.unwrap_err().code(), tonic::Code::NotFound);
}