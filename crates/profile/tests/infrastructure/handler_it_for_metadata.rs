// crates/profile/tests/infrastructure/handler_it_for_metadata.rs

use std::sync::Arc;
use tonic::Request;

use shared_kernel::domain::value_objects::{AccountId, RegionCode};
use profile::infrastructure::api::grpc::handlers::MetadataHandler;
use profile::infrastructure::api::grpc::profile_v1::{
    UpdateBioRequest, UpdateLocationLabelRequest, UpdateSocialLinksRequest,
    SocialLinks as ProtoSocialLinks
};
use profile::infrastructure::api::grpc::profile_v1::profile_metadata_service_server::ProfileMetadataService;

use profile::application::use_cases::update_bio::UpdateBioUseCase;
use profile::application::use_cases::update_location_label::UpdateLocationLabelUseCase;
use profile::application::use_cases::update_social_links::UpdateSocialLinksUseCase;
use profile::domain::entities::Profile;
use profile::domain::repositories::{ProfileIdentityRepository, ProfileRepository};
use profile::domain::value_objects::{DisplayName, Handle, ProfileId};
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;

use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::repositories::RedisCacheRepository;
use shared_kernel::infrastructure::utils::InfrastructureKernelTestContext;

// --- UTILS DE SETUP ---

struct MetaDataTestContext {
    handler: MetadataHandler,
    infra: InfrastructureKernelTestContext,
    identity_repo: Arc<PostgresIdentityRepository>,
    composite_repo: Arc<UnifiedProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    profile_id: ProfileId,
    region: RegionCode,
}

async fn setup_test_context() -> MetaDataTestContext {
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

    // 4. Injection dans les Use Cases et le Handler
    let handler = MetadataHandler::new(
        Arc::new(UpdateBioUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateLocationLabelUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateSocialLinksUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    // 5. Seed initial du profil de test
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = Profile::builder(
        owner_id,
        region.clone(),
        DisplayName::try_new("Metadata User").unwrap(),
        Handle::try_new("meta_pro").unwrap(),
    ).build();

    let profile_id = initial_profile.id().clone();
    profile_repo.save_identity(&initial_profile, None, None)
        .await
        .expect("Failed to seed profile");

    MetaDataTestContext {
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
async fn test_metadata_handler_bio_lifecycle() {
    let ctx = setup_test_context().await;
    let bio_text = "Software Architect & Rust Enthusiast";

    // 1. UPDATE BIO
    let mut req = Request::new(UpdateBioRequest {
        profile_id: ctx.profile_id.to_string(),
        new_bio: Some(bio_text.into()),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_bio(req).await.expect("Update bio failed");
    assert_eq!(response.into_inner().bio, Some(bio_text.into()));

    // 2. CLEAR BIO (Test optionnalité)
    let mut clear_req = Request::new(UpdateBioRequest {
        profile_id: ctx.profile_id.to_string(),
        new_bio: None,
    });
    clear_req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_bio(clear_req).await.expect("Clear bio failed");
    assert_eq!(response.into_inner().bio, None);
}

#[tokio::test]
async fn test_metadata_handler_location_label_validation() {
    let ctx = setup_test_context().await;
    let location = "Paris, France";

    // 1. Success path
    let mut req = Request::new(UpdateLocationLabelRequest {
        profile_id: ctx.profile_id.to_string(),
        new_location_label: Some(location.into()),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_location_label(req).await.expect("Update location failed");
    assert_eq!(response.into_inner().location_label, Some(location.into()));

    // 2. Validation path (Label trop long)
    let long_label = "a".repeat(101); // Limite fixée à 100 dans ton LocationLabel
    let mut fail_req = Request::new(UpdateLocationLabelRequest {
        profile_id: ctx.profile_id.to_string(),
        new_location_label: Some(long_label),
    });
    fail_req.extensions_mut().insert(ctx.region.clone());

    let result = ctx.handler.update_location_label(fail_req).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_metadata_handler_social_links_persistence() {
    let ctx = setup_test_context().await;

    let links = ProtoSocialLinks {
        x_url: Some("https://x.com/rust_dev".into()),
        github_url: Some("https://github.com/rust_dev".into()),
        ..Default::default()
    };

    let mut req = Request::new(UpdateSocialLinksRequest {
        profile_id: ctx.profile_id.to_string(),
        new_links: Some(links),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_social_links(req).await.expect("Update social links failed");
    let proto = response.into_inner();

    let social = proto.social_links.expect("Social links should be present");
    assert_eq!(social.x_url, Some("https://x.com/rust_dev".into()));
    assert_eq!(social.github_url, Some("https://github.com/rust_dev".into()));

    // Vérification de la persistance réelle
    let profile = ctx.identity_repo.fetch(&ctx.profile_id, &ctx.region).await.unwrap().unwrap();
    assert!(profile.social_links().is_some());
}

#[tokio::test]
async fn test_metadata_handler_region_missing_status() {
    let ctx = setup_test_context().await;

    // Erreur volontaire : on n'injecte PAS la région dans les extensions
    let request = Request::new(UpdateBioRequest {
        profile_id: ctx.profile_id.to_string(),
        new_bio: Some("Hello".into()),
    });

    let result = ctx.handler.update_bio(request).await;

    assert!(result.is_err());
    // L'absence de RegionCode dans les extensions est une erreur serveur (Internal)
    // car cela signifie que l'intercepteur n'a pas fait son job.
    assert_eq!(result.unwrap_err().code(), tonic::Code::Internal);
}