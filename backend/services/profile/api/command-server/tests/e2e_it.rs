// backend/services/profile/api/command-server/tests/profile_e2e_test.rs (ou ton chemin de test)

use auth::Claims;
use auth_test_utils::TokenValidatorStub;
use profile_old::entities::Profile;
use profile_old::repositories::{ProfileRepository, ProfileRoutingRepository};
use profile_old::stores::{ScyllaProfileRoutingStore, ScyllaProfileStore};
use profile_old::types::{DisplayName, Handle};
use profile_test_utils::ProfileTestContextBuilder;
use shared_kernel::core::{Result, Versioned};
use shared_kernel::types::{AccountId, ProfileId, Region};
use shared_proto::profile::v1::profile_identity_service_client::ProfileIdentityServiceClient;
use shared_proto::profile::v1::{ChangeHandleRequest, ProfileTarget};
use tonic::Request;
use tonic::metadata::MetadataValue;
use uuid::Uuid;

fn with_auth<T>(payload: T, token: &str, region: &str) -> Request<T> {
    let mut request = Request::new(payload);
    let token_val = format!("Bearer {}", token)
        .parse::<MetadataValue<_>>()
        .unwrap();
    request.metadata_mut().insert("authorization", token_val);
    request
        .metadata_mut()
        .insert("x-region", region.parse().unwrap());
    request
}

#[tokio::test]
async fn test_e2e_complete_profile_lifecycle() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let test_token = "simulated.profile.service.jwt";
    let target_sub_id = "keycloak|profile-user-123";
    let mock_validator = std::sync::Arc::new(TokenValidatorStub::new());

    let expected_claims = Claims {
        sub_id: shared_kernel::types::SubId::try_new(target_sub_id)?,
        aud: serde_json::Value::String("profile-service".to_string()),
        iss: "https://identity.core.platform/realms/master".to_string(),
        email: None,
        email_verified: None,
        phone_number: None,
        phone_number_verified: None,
        realm_access: None,
        exp: chrono::Utc::now().timestamp() as u64 + 3600,
    };

    mock_validator.stub_token(test_token, expected_claims);

    // 1. SETUP INFRASTRUCTURE
    let ctx = ProfileTestContextBuilder::new()
        .with_mock_auth(mock_validator)
        .with_grpc_server()
        .build_e2e()
        .await;

    let mut identity_client = ProfileIdentityServiceClient::connect(ctx.grpc_url())
        .await
        .unwrap();

    // 2. SETUP DONNÉES DE TEST via les Stores de Production
    let region = Region::default();
    let region_str = region.as_str();
    let real_profile_id = ProfileId::generate();
    let real_account_id = AccountId::generate();
    let handle = Handle::try_new("alice_rocks")?;

    let session = ctx.kernel().scylla().session();
    let keyspace_name = format!("{}_profile_storage", region.to_string().to_lowercase());

    let profile_store = ScyllaProfileStore::new(session.clone(), keyspace_name)
        .await
        .unwrap();

    let routing_store = ScyllaProfileRoutingStore::new(session.clone())
        .await
        .unwrap();

    let mut profile = Profile::builder(real_account_id, real_profile_id, handle.clone())?
        .with_display_name(DisplayName::try_new("Alice")?)
        .build()?;

    profile_store.save(&mut profile).await.unwrap();
    routing_store
        .register_routing(real_profile_id, &handle.to_sha256_hash(), region)
        .await
        .unwrap();

    // 3. EXÉCUTION DU TEST E2E via gRPC
    let change_handle_req = ChangeHandleRequest {
        command_id: Uuid::now_v7().to_string(),
        target: Some(ProfileTarget {
            profile_id: real_profile_id.to_string(),
            expected_version: 0,
        }),
        new_handle: "alice_wonderland".to_string(),
    };

    let res = identity_client
        .change_handle(with_auth(change_handle_req, test_token, region_str))
        .await;

    assert!(
        res.is_ok(),
        "Le changement de handle a échoué : {:?}",
        res.err()
    );

    // 4. ASSERTION via le Store de production
    let saved_profile = profile_store
        .find_by_id(real_profile_id)
        .await
        .unwrap()
        .expect("Profile should exist in ScyllaDB");

    assert_eq!(saved_profile.handle().as_str(), "alice_wonderland");
    assert_eq!(saved_profile.version(), 1);

    ctx.shutdown().await;
    Ok(())
}
