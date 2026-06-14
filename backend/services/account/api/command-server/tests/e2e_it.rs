// backend/services/account/api/command-server/tests/e2e_it.rs

use account_test_utils::AccountTestContextBuilder;
use auth::Claims;
use auth_test_utils::TokenValidatorStub;
use shared_kernel::core::Result;
use shared_kernel::types::SubId;
use shared_proto::account::v1::AccountTarget;
use shared_proto::account::v1::account_personal_service_client::AccountPersonalServiceClient;
use shared_proto::account::v1::account_registration_service_client::AccountRegistrationServiceClient;

use infra_sqlx::sqlx;
use tonic::{Request, metadata::MetadataValue};
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;

use shared_proto::account::v1::{
    RegisterRequest, RegistrationIdentifier, UpdateLocaleRequest, registration_identifier::Method,
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
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=debug,account=debug,tonic=debug")),
        )
        .with_test_writer()
        .try_init();

    tracing::info!("Démarrage du test E2E de cycle de vie de compte (Isolé via Mock)");

    let region: &str = "EU";
    let test_token = "simulated.valid.jwt.token";
    let target_sub_id = "keycloak|user-e2e-123456";
    let mock_validator = std::sync::Arc::new(TokenValidatorStub::new());

    let expected_claims = Claims {
        sub_id: SubId::try_new(target_sub_id)?,
        aud: serde_json::Value::String("account-service".to_string()),
        iss: "https://identity.core.platform/realms/master".to_string(),

        email: None,
        email_verified: None,
        phone_number: None,
        phone_number_verified: None,
        realm_access: None,
        exp: chrono::Utc::now().timestamp() as u64 + 3600,
    };

    // On dit au validateur d'accepter notre chaîne de caractères arbitraire
    mock_validator.stub_token(test_token, expected_claims);

    // 2. SETUP INFRASTRUCTURE (Bases de données réelles en Docker, mais Auth mockée)
    let ctx = AccountTestContextBuilder::new()
        .with_mock_auth(mock_validator)
        .with_grpc_server()
        .build()
        .await;

    // 3. CLIENTS gRPC
    let grpc_url = ctx.grpc_url();

    let mut registration_client = AccountRegistrationServiceClient::connect(grpc_url.clone())
        .await
        .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

    let mut personal_client = AccountPersonalServiceClient::connect(grpc_url)
        .await
        .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

    let register_command_id = Uuid::now_v7().to_string();

    // 4. PRÉPARATION PAYLOAD
    let register_payload: RegisterRequest = RegisterRequest {
        command_id: register_command_id.clone(),
        sub_id: Some(target_sub_id.to_string()),
        identifier: Some(RegistrationIdentifier {
            method: Some(Method::Email("audit-e2e@test.com".to_string())),
        }),
        locale: "fr-FR".to_string(),
        ip_addr: "127.0.0.1".to_string(),
    };

    // 5. ACT : Inscription via le Registration Client
    let res = registration_client
        .register(with_auth(register_payload.clone(), test_token, region))
        .await;
    assert!(res.is_ok(), "L'inscription a échoué: {:?}", res.err());

    let real_account_id = res.unwrap().into_inner().account_id;

    // Idempotence sur le canal de Registration
    let res_dup = registration_client
        .register(with_auth(register_payload, test_token, region))
        .await;
    assert!(res_dup.is_ok(), "L'idempotence de l'inscription a échoué");

    // 6. ACT : Update locale (Zone gRPC privée sécurisée par AuthInterceptor)
    let account_id_uuid = Uuid::parse_str(&real_account_id).unwrap();

    // On vérifie que l'écriture a bien eu lieu dans le pool Postgres régional du TestContext
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
        .update_locale(with_auth(update_payload, test_token, region))
        .await;
    assert!(
        upd_res.is_ok(),
        "La mise à jour de la locale a échoué : {:?}",
        upd_res.err()
    );

    // 7. VÉRIFICATIONS FINALES EN BASE DE DONNÉES
    let row: (String, i64) = sqlx::query_as(
        "SELECT locale, version FROM account_identity WHERE account_id = $1 AND region = $2",
    )
    .bind(account_id_uuid)
    .bind(region)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Profile should exist in DB");

    assert_eq!(row.0, "en-US");
    assert_eq!(row.1, 2);

    // 8. SHUTDOWN DOCKER CLEANUP (Postgres et Redis s'éteignent proprement)
    ctx.shutdown().await;
    Ok(())
}
