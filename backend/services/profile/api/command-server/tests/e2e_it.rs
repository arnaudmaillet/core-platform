// backend/services/profile/api/command-server/tests/e2e_it.rs

use auth_test_utils::KeycloakTestContext;
use infra_sqlx::sqlx;
use profile_test_utils::ProfileTestContextBuilder;
use shared_kernel::core::{Identifier, Result};
use shared_kernel::types::{AccountId, ProfileId, Region};
use shared_proto::profile::v1::profile_identity_service_client::ProfileIdentityServiceClient;
use shared_proto::profile::v1::{ChangeHandleRequest, ProfileTarget};
use tonic::Request;
use tonic::metadata::MetadataValue;
use uuid::Uuid;

// Helper Auth resté ici car spécifique au client du test
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

    let ctx = ProfileTestContextBuilder::new()
        .with_grpc_server()
        .build_e2e()
        .await;

    let mut identity_client = ProfileIdentityServiceClient::connect(ctx.grpc_url())
        .await
        .unwrap();

    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let region = Region::default();
    let region_str = region.as_str();
    let real_profile_id = ProfileId::generate();
    let real_account_id = AccountId::generate();

    sqlx::query(
        "INSERT INTO user_profiles (profile_id, account_id, region, handle, display_name, version, is_private, created_at, updated_at) 
         VALUES ($1, $2, $3, $4, $5, 0, false, NOW(), NOW())"
    )
    .bind(real_profile_id.as_uuid())
    .bind(real_account_id.uuid())
    .bind(region_str)
    .bind("alice_rocks")
    .bind("Alice")
    .execute(&ctx.pg_pool())
    .await
    .unwrap();

    let change_handle_req = ChangeHandleRequest {
        command_id: Uuid::now_v7().to_string(),
        target: Some(ProfileTarget {
            profile_id: real_profile_id.to_string(),
            region: region_str.to_string(),
            expected_version: 0,
        }),
        new_handle: "alice_wonderland".to_string(),
    };

    let res = identity_client
        .change_handle(with_auth(
            change_handle_req,
            &auth_response.token.as_str(),
            region_str,
        ))
        .await;

    assert!(
        res.is_ok(),
        "Le changement de handle a échoué : {:?}",
        res.err()
    );

    let row: (String, i64) = sqlx::query_as(
        "SELECT handle, version FROM user_profiles WHERE profile_id = $1 AND region = $2",
    )
    .bind(real_profile_id.as_uuid())
    .bind(region_str)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Profile should exist in DB");

    assert_eq!(row.0, "alice_wonderland");
    assert_eq!(row.1, 1);

    ctx.shutdown().await;
    Ok(())
}
