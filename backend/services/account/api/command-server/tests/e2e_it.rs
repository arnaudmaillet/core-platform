// backend/services/account/api/command-server/tests/e2e_it.rs

use account_test_utils::AccountTestContextBuilder;
use infra_test::KeycloakTestContext;
use shared_kernel::core::Result;
use shared_proto::account::v1::AccountTarget;
use shared_proto::account::v1::account_personal_service_client::AccountPersonalServiceClient;

use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;
use infra_sqlx::sqlx;

use auth::TokenValidator;
use shared_proto::account::v1::{
    RegisterRequest, RegistrationIdentifier, UpdateLocaleRequest,
    account_access_service_client::AccountAccessServiceClient, registration_identifier::Method,
};

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
async fn test_e2e_complete_account_lifecycle() -> Result<()> {
    // 1. SETUP
    let ctx = AccountTestContextBuilder::new()
        .with_grpc_server()
        .build()
        .await;

    // 2. CLIENTS gRPC
    let grpc_url = ctx.grpc_url();
    let mut access_client = AccountAccessServiceClient::connect(grpc_url.clone())
        .await
        .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?; // Conversion explicite

    let mut personal_client = AccountPersonalServiceClient::connect(grpc_url)
        .await
        .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

    // 3. AUTH & IDENTITY
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;
    let claims = auth_ctx
        .validator
        .validate(&auth_response.token)
        .expect("Token invalid");

    let region: &str = "EU";
    let register_command_id = Uuid::now_v7().to_string();

    // 4. PRÉPARATION PAYLOAD
    let register_payload = RegisterRequest {
        command_id: register_command_id.clone(),
        sub_id: Some(claims.sub_id.to_string()),
        identifier: Some(RegistrationIdentifier {
            method: Some(Method::Email("audit-e2e@test.com".to_string())),
        }),
        region: region.to_string(),
        locale: "fr-FR".to_string(),
        ip_addr: "127.0.0.1".to_string(),
    };

    // 5. ACT : Register
    let res = access_client
        .register(with_auth(
            register_payload.clone(),
            &auth_response.token.as_str(),
            region,
        ))
        .await;
    assert!(res.is_ok(), "Le registre a échoué: {:?}", res.err());

    let real_account_id = res.unwrap().into_inner().account_id;

    // Idempotence
    let res_dup = access_client
        .register(with_auth(
            register_payload,
            &auth_response.token.as_str(),
            region,
        ))
        .await;
    assert!(res_dup.is_ok());

    // 6. ACT : Update locale
    let account_id_uuid = Uuid::parse_str(&real_account_id).unwrap();

    // On récupère la version actuelle depuis la DB
    let current_version: i64 = sqlx::query_scalar(
        "SELECT version FROM account_identity WHERE account_id = $1 AND region = $2",
    )
    .bind(account_id_uuid)
    .bind(region)
    .fetch_one(&ctx.pg_pool())
    .await
    .unwrap();

    let update_payload = UpdateLocaleRequest {
        command_id: Uuid::now_v7().to_string(),
        target: Some(AccountTarget {
            account_id: real_account_id,
            region: region.to_string(),
            expected_version: current_version as u64,
        }),
        locale: "en-US".to_string(),
    };

    let upd_res = personal_client
        .update_locale(with_auth(
            update_payload,
            &auth_response.token.as_str(),
            region,
        ))
        .await;
    assert!(upd_res.is_ok());

    // 7. VÉRIFICATIONS FINALES
    let row: (String, i64) = sqlx::query_as(
        "SELECT locale, version FROM account_identity WHERE account_id = $1 AND region = $2",
    )
    .bind(account_id_uuid)
    .bind(region)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Profile should exist in DB");

    assert_eq!(row.0, "en-US");
    assert_eq!(row.1, 2); // Version incrémentée

    // 8. SHUTDOWN
    ctx.shutdown().await;
    Ok(())
}
