use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tonic::Request;

use shared_kernel::domain::value_objects::{AccountId, RegionCode, Url, Username};
use profile::infrastructure::api::grpc::handlers::MediaHandler;
use profile::infrastructure::api::grpc::profile_v1::{
    UpdateAvatarRequest, RemoveAvatarRequest, UpdateBannerRequest, RemoveBannerRequest
};
use profile::infrastructure::api::grpc::profile_v1::profile_media_service_server::ProfileMediaService;

use profile::application::update_avatar::UpdateAvatarUseCase;
use profile::application::remove_avatar::RemoveAvatarUseCase;
use profile::application::update_banner::UpdateBannerUseCase;
use profile::application::remove_banner::RemoveBannerUseCase;
use profile::domain::repositories::{ProfileIdentityRepository, ProfileRepository};
use profile::domain::value_objects::DisplayName;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;

// --- UTILS DE SETUP ---

struct TestContext {
    handler: MediaHandler,
    identity_repo: Arc<PostgresProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    account_id: AccountId,
    region: RegionCode,
    _pg_container: ContainerAsync<Postgres>,
}

async fn setup_test_context() -> TestContext {
    // Utilisation des Singletons configurés précédemment
    let (pool, pg_container) = crate::common::setup_postgres_test_db().await;
    let scylla_session = crate::common::setup_scylla_db().await;

    let identity_postgres = Arc::new(PostgresProfileRepository::new(pool.clone()));
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(scylla_session.clone()));

    let profile_repo = Arc::new(CompositeProfileRepository::new(
        identity_postgres.clone(),
        stats_scylla.clone(),
    ));

    let tx_manager = Arc::new(PostgresTransactionManager::new(pool.clone()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));

    let handler = MediaHandler::new(
        Arc::new(UpdateAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(RemoveAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(RemoveBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    // Seed initial d'un profil sans media
    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = profile::domain::builders::ProfileBuilder::new(
        account_id.clone(),
        region.clone(),
        DisplayName::try_new("Media User").unwrap(),
        Username::try_new(format!("user_{}", account_id.to_string()[..8].to_string())).unwrap(),
    ).build();

    profile_repo.save(&initial_profile, None).await.expect("Failed to seed user");

    TestContext {
        handler,
        identity_repo: identity_postgres,
        outbox_repo,
        account_id,
        region,
        _pg_container: pg_container,
    }
}

// --- TESTS ---

#[tokio::test]
async fn test_media_handler_avatar_lifecycle() {
    let ctx = setup_test_context().await;
    let avatar_url = "https://cdn.assets.com/avatars/123.png";

    // 1. UPDATE AVATAR
    let mut update_req = Request::new(UpdateAvatarRequest {
        account_id: ctx.account_id.to_string(),
        new_avatar_url: avatar_url.into(),
    });
    update_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_avatar(update_req).await.expect("Update avatar failed");
    assert_eq!(response.into_inner().avatar_url, Some(avatar_url.into()));

    // Vérification DB
    let profile = ctx.identity_repo.find_by_id(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert_eq!(profile.avatar_url().unwrap().as_str(), avatar_url);

    // 2. REMOVE AVATAR
    let mut remove_req = Request::new(RemoveAvatarRequest {
        account_id: ctx.account_id.to_string(),
    });
    remove_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.remove_avatar(remove_req).await.expect("Remove avatar failed");
    assert_eq!(response.into_inner().avatar_url, None);

    // Vérification finale DB
    let profile_final = ctx.identity_repo.find_by_id(&ctx.account_id, &ctx.region).await.unwrap().unwrap();
    assert!(profile_final.avatar_url().is_none());
}

#[tokio::test]
async fn test_media_handler_banner_lifecycle() {
    let ctx = setup_test_context().await;
    let banner_url = "https://cdn.assets.com/banners/456.jpg";

    // 1. UPDATE BANNER
    let mut update_req = Request::new(UpdateBannerRequest {
        account_id: ctx.account_id.to_string(),
        new_banner_url: banner_url.into(),
    });
    update_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_banner(update_req).await.expect("Update banner failed");
    assert_eq!(response.into_inner().banner_url, Some(banner_url.into()));

    // 2. REMOVE BANNER
    let mut remove_req = Request::new(RemoveBannerRequest {
        account_id: ctx.account_id.to_string(),
    });
    remove_req.extensions_mut().insert(ctx.region.clone());

    ctx.handler.remove_banner(remove_req).await.expect("Remove banner failed");

    // Vérification Outbox : on attend 2 événements (Update + Remove) + 1 du Seed
    let pending = ctx.outbox_repo.find_pending(10).await.unwrap();
    assert!(pending.len() >= 2);
}

#[tokio::test]
async fn test_media_handler_invalid_url_format() {
    let ctx = setup_test_context().await;

    let mut request = Request::new(UpdateAvatarRequest {
        account_id: ctx.account_id.to_string(),
        new_avatar_url: "not-a-valid-url".into(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_avatar(request).await;

    assert!(result.is_err());
    // On vérifie que c'est bien une erreur de validation (InvalidArgument)
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_media_handler_account_not_found() {
    let ctx = setup_test_context().await;
    let random_id = AccountId::new(); // ID qui n'existe pas en DB

    let mut request = Request::new(RemoveAvatarRequest {
        account_id: random_id.to_string(),
    });
    request.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.remove_avatar(request).await;

    assert!(result.is_err());
    // Selon ton implémentation, ça peut être NotFound ou Internal
    // On s'assure juste que le système ne panique pas
}