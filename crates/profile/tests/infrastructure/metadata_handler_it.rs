use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tonic::Request;

use shared_kernel::domain::value_objects::{AccountId, RegionCode, LocationLabel};
use profile::infrastructure::api::grpc::handlers::MetadataHandler;
use profile::infrastructure::api::grpc::profile_v1::{
    UpdateBioRequest, UpdateLocationLabelRequest, UpdateSocialLinksRequest,
    SocialLinks as ProtoSocialLinks
};
use profile::infrastructure::api::grpc::profile_v1::profile_metadata_service_server::ProfileMetadataService;

use profile::application::update_bio::UpdateBioUseCase;
use profile::application::update_location_label::UpdateLocationLabelUseCase;
use profile::application::update_social_links::UpdateSocialLinksUseCase;
use profile::domain::entities::Profile;
use profile::domain::repositories::ProfileRepository;
use profile::domain::value_objects::DisplayName;
use shared_kernel::domain::value_objects::Username;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;
use shared_kernel::domain::repositories::OutboxRepository;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;

// --- UTILS DE SETUP ---

struct TestContext {
    handler: MetadataHandler,
    identity_repo: Arc<PostgresProfileRepository>,
    outbox_repo: Arc<PostgresOutboxRepository>,
    account_id: AccountId,
    region: RegionCode,
    _pg_container: ContainerAsync<Postgres>,
}

async fn setup_test_context() -> TestContext {
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

    let handler = MetadataHandler::new(
        Arc::new(UpdateBioUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateLocationLabelUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
        Arc::new(UpdateSocialLinksUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone())),
    );

    let account_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let initial_profile = Profile::builder(
        account_id.clone(),
        region.clone(),
        DisplayName::try_new("Metadata User").unwrap(),
        Username::try_new(format!("meta_{}", account_id.to_string()[..8].to_string())).unwrap(),
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
async fn test_metadata_handler_bio_lifecycle() {
    let ctx = setup_test_context().await;
    let bio_text = "Software Architect & Rust Enthusiast";

    // 1. UPDATE BIO (Success)
    let mut req = Request::new(UpdateBioRequest {
        account_id: ctx.account_id.to_string(),
        new_bio: Some(bio_text.into()),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_bio(req).await.expect("Update bio failed");
    assert_eq!(response.into_inner().bio, Some(bio_text.into()));

    // 2. CLEAR BIO (Optionality test)
    let mut clear_req = Request::new(UpdateBioRequest {
        account_id: ctx.account_id.to_string(),
        new_bio: None, // Ou string vide selon ton filter
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
        account_id: ctx.account_id.to_string(),
        new_location_label: Some(location.into()),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_location_label(req).await.expect("Update location failed");
    assert_eq!(response.into_inner().location_label, Some(location.into()));

    // 2. Validation path (Too long label)
    let long_label = "a".repeat(501); // Supposons une limite à 500
    let mut fail_req = Request::new(UpdateLocationLabelRequest {
        account_id: ctx.account_id.to_string(),
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
        x_url: Some("https://twitter.com/rust_dev".into()),
        instagram_url: None,
        facebook_url: None,
        tiktok_url: None,
        youtube_url: None,
        twitch_url: None,
        discord_url: None,
        onlyfans_url: None,
        github_url: Some("https://github.com/rust_dev".into()),
        website_url: None,
        linkedin_url: None,
        others: Default::default(),
    };

    let mut req = Request::new(UpdateSocialLinksRequest {
        account_id: ctx.account_id.to_string(),
        new_links: Some(links),
    });
    req.extensions_mut().insert(ctx.region.clone());

    let response = ctx.handler.update_social_links(req).await.expect("Update social links failed");
    let proto = response.into_inner();

    let social = proto.social_links.expect("Social links should be present");
    assert_eq!(social.x_url, Some("https://twitter.com/rust_dev".into()));
    assert_eq!(social.github_url, Some("https://github.com/rust_dev".into()));

    // Vérification Outbox pour s'assurer que l'événement MetadataChanged est produit
    let pending = ctx.outbox_repo.find_pending(10).await.unwrap();
    assert!(!pending.is_empty(), "Outbox should contain the social links update event");
}

#[tokio::test]
async fn test_metadata_handler_region_missing_status() {
    let ctx = setup_test_context().await;

    // On n'injecte PAS la région dans les extensions
    let request = Request::new(UpdateBioRequest {
        account_id: ctx.account_id.to_string(),
        new_bio: Some("Hello".into()),
    });

    let result = ctx.handler.update_bio(request).await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::Internal);
}