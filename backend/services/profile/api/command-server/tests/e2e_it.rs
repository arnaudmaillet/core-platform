// backend/services/profile/api/command-server/tests/e2e_it.rs

use std::net::SocketAddr;
use tonic::{Request, Code};
use tonic::metadata::MetadataValue;

use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_client::ProfileIdentityServiceClient;
use profile::infrastructure::api::grpc::profile_v1::profile_metadata_service_client::ProfileMetadataServiceClient;
use profile::infrastructure::api::grpc::profile_v1::profile_media_service_client::ProfileMediaServiceClient;
use profile::infrastructure::api::grpc::profile_v1::{UpdateHandleRequest, UpdateBioRequest, UpdateAvatarRequest};

use profile::domain::entities::Profile;
use profile::domain::repositories::ProfileIdentityRepository;
use profile::domain::value_objects::{DisplayName, Handle};
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;
use profile::infrastructure::utils::InfrastructureProfileTestContext;
use shared_kernel::domain::value_objects::{AccountId, RegionCode};

#[path = "../src/main.rs"]
mod server_binary;

async fn start_test_server(infra: &InfrastructureProfileTestContext) -> String {
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = std::net::TcpListener::bind(addr).unwrap();
    let actual_addr = listener.local_addr().unwrap();
    drop(listener);

    unsafe {
        std::env::set_var("PROFILE_DB_URL", infra.kernel().postgres().url());
        std::env::set_var("PROFILE_SCYLLA_NODES", infra.kernel().scylla().uri());
        std::env::set_var("PROFILE_SCYLLA_KEYSPACE", infra.kernel().scylla().keyspace());
        std::env::set_var("PROFILE_REDIS_URL", infra.kernel().redis().url());
    }

    tokio::spawn(async move {
        server_binary::run_server(actual_addr)
            .await
            .expect("Server failed");
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    format!("http://{}", actual_addr)
}

#[tokio::test]
async fn test_profile_e2e_comprehensive() {
    let infra = InfrastructureProfileTestContext::setup().await;
    let server_url = start_test_server(&infra).await;

    // Initialisation des clients
    let mut identity_client = ProfileIdentityServiceClient::connect(server_url.clone()).await.unwrap();
    let mut metadata_client = ProfileMetadataServiceClient::connect(server_url.clone()).await.unwrap();
    let mut media_client = ProfileMediaServiceClient::connect(server_url.clone()).await.unwrap();

    // --- SEED INITIAL ---
    let owner_id = AccountId::new();
    let region = RegionCode::try_new("eu").unwrap();
    let profile = Profile::builder(
        owner_id.clone(),
        region.clone(),
        DisplayName::try_new("Initial Name").unwrap(),
        Handle::try_new("initial_handle").unwrap(),
    ).build();

    let pg_repo = PostgresIdentityRepository::new(infra.kernel().postgres().pool());
    pg_repo.save(&profile, None).await.expect("Seed failed");

    // --- CASE 1: IDENTITY SERVICE (IdentityHandler) ---
    let mut req = Request::new(UpdateHandleRequest {
        profile_id: profile.id().to_string(),
        new_handle: "new_handle_ok".into(),
    });
    req.metadata_mut().insert("x-region", MetadataValue::from_static("eu"));
    let res = identity_client.update_handle(req).await.expect("Identity service failed");
    assert_eq!(res.into_inner().handle, "new_handle_ok");

    // --- CASE 2: METADATA SERVICE (MetadataHandler) ---
    // Vérifie que le MetadataHandler est bien branché dans le main.rs
    let mut req_bio = Request::new(UpdateBioRequest {
        profile_id: profile.id().to_string(),
        new_bio: Some("Hello, I am a test profile".into()),
    });
    req_bio.metadata_mut().insert("x-region", MetadataValue::from_static("eu"));
    let res_bio = metadata_client.update_bio(req_bio).await.expect("Metadata service failed");
    assert_eq!(res_bio.into_inner().bio, Some("Hello, I am a test profile".into()));

    // --- CASE 3: MEDIA SERVICE (MediaHandler) ---
    // Vérifie que le MediaHandler est bien branché dans le main.rs
    let mut req_avatar = Request::new(UpdateAvatarRequest {
        profile_id: profile.id().to_string(),
        new_avatar_url: "https://cdn.test.com/avatar.png".into(),
    });
    req_avatar.metadata_mut().insert("x-region", MetadataValue::from_static("eu"));
    let res_avatar = media_client.update_avatar(req_avatar).await.expect("Media service failed");
    assert_eq!(res_avatar.into_inner().avatar_url, Some("https://cdn.test.com/avatar.png".into()));

    // --- CASE 4: ERRORS & SECURITY ---
    // Missing Region Header
    let req_no_header = Request::new(UpdateHandleRequest {
        profile_id: profile.id().to_string(),
        new_handle: "fail".into(),
    });
    let err = identity_client.update_handle(req_no_header).await.unwrap_err();
    assert_eq!(err.code(), Code::InvalidArgument);

    // Not Found
    let mut req_not_found = Request::new(UpdateHandleRequest {
        profile_id: uuid::Uuid::new_v4().to_string(),
        new_handle: "ghost".into(),
    });
    req_not_found.metadata_mut().insert("x-region", MetadataValue::from_static("eu"));
    let res_err = identity_client.update_handle(req_not_found).await.unwrap_err();
    assert_eq!(res_err.code(), Code::NotFound);

    // --- FINAL DB VERIFICATION ---
    // On vérifie que toutes les mutations (Handle + Bio + Avatar) sont bien persistées
    let db_profile = pg_repo.fetch(profile.id(), &region).await.unwrap().unwrap();
    assert_eq!(db_profile.handle().as_str(), "new_handle_ok");
    assert_eq!(db_profile.bio().map(|b| b.as_str()), Some("Hello, I am a test profile"));
    assert_eq!(db_profile.avatar_url().map(|a| a.as_str()), Some("https://cdn.test.com/avatar.png"));
}