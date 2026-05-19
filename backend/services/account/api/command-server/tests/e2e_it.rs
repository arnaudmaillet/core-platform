// backend/services/account/api/command-server/tests/e2e_it.rs

use account::services::{AccountAccessService, AccountPersonalService};
use account::test_utils::AccountTestContext;
use shared_kernel::core::Result;
use shared_proto::account::v1::AccountTarget;
use shared_proto::account::v1::account_personal_service_client::AccountPersonalServiceClient;
use shared_proto::account::v1::account_personal_service_server::AccountPersonalServiceServer;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, metadata::MetadataValue};
use uuid::Uuid;

// Imports de ton architecture
use auth::{AuthInterceptor, KeycloakTestContext, KeycloakValidator, TokenValidator};
use shared_proto::account::v1::{
    RegisterRequest, RegistrationIdentifier, UpdateLocaleRequest,
    account_access_service_client::AccountAccessServiceClient,
    account_access_service_server::AccountAccessServiceServer, registration_identifier::Method,
};

use account::AccountServiceBuilder;

// --- HELPER POUR LE TOKEN ---
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
    // 1. SETUP INFRA & SERVEUR GRPC VIA CLOSURE
    let ctx = AccountTestContext::builder()
        .with_server(
            |pg_pool, redis_repo, _kafka_brokers, addr, shutdown_rx, ready_tx| {
                async move {
                    // 1. Setup Auth
                    let auth_ctx = KeycloakTestContext::restore("master").await;
                    let validator = Arc::new(
                        KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                            .await
                            .unwrap(),
                    );
                    let interceptor = AuthInterceptor::new(validator);

                    // 2. Setup Domaine
                    let builder = AccountServiceBuilder::new(pg_pool, redis_repo);
                    let app_ctx = builder.build_context();
                    let bus = builder.build_command_bus();

                    // 3. Instanciation des services gRPC
                    let access_svc = AccountAccessService::new(bus.clone(), app_ctx.clone());
                    let personal_svc = AccountPersonalService::new(bus, app_ctx);
                    ready_tx.send(()).ok();

                    // 4. Lancement du serveur gRPC unique Tonic
                    Server::builder()
                        .add_service(AccountAccessServiceServer::with_interceptor(
                            access_svc,
                            interceptor.clone(),
                        ))
                        .add_service(AccountPersonalServiceServer::with_interceptor(
                            personal_svc,
                            interceptor,
                        ))
                        .serve_with_shutdown(addr, async {
                            shutdown_rx.await.ok();
                        })
                        .await
                        .unwrap();
                }
            },
        )
        .build_e2e()
        .await;

    let mut access_client = AccountAccessServiceClient::connect(ctx.kernel().grpc_url())
        .await
        .unwrap();
    let mut personal_client = AccountPersonalServiceClient::connect(ctx.kernel().grpc_url())
        .await
        .unwrap();

    // 2. AUTH & IDENTITY EXTRACTION
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;
    let claims = auth_ctx
        .validator
        .validate(&auth_response.token)
        .expect("Token invalid");

    let true_sub_id = claims.sub_id.to_string();
    let _sub_uuid = Uuid::parse_str(&true_sub_id).expect("Invalid UUID from Keycloak");
    let region: &str = "EU";

    // --- ÉTAPE 1 : REGISTER (v0 -> v1) ---
    let register_command_id = Uuid::now_v7().to_string();
    let register_payload = RegisterRequest {
        command_id: register_command_id.clone(),
        sub_id: Some(true_sub_id.clone()),
        identifier: Some(RegistrationIdentifier {
            method: Some(Method::Email("audit-e2e@test.com".to_string())),
        }),
        region: region.to_string(),
        locale: "fr-FR".to_string(),
        ip_addr: "127.0.0.1".to_string(),
    };

    let res = access_client
        .register(with_auth(
            register_payload.clone(),
            &auth_response.token.as_str(),
            region,
        ))
        .await;

    assert!(res.is_ok(), "First register failed: {:?}", res.err());

    let created_identity = res.unwrap().into_inner();
    let real_account_id = created_identity.account_id;

    // --- ÉTAPE 2 : IDEMPOTENCE TECHNIQUE ---
    let res_dup = access_client
        .register(with_auth(
            register_payload,
            &auth_response.token.as_str(),
            region,
        ))
        .await;

    match &res_dup {
        Ok(_) => println!("DEBUG E2E: Idempotency call SUCCESS"),
        Err(e) => println!(
            "DEBUG E2E: Idempotency call FAILED with Code: {:?}, Msg: {}",
            e.code(),
            e.message()
        ),
    }

    assert!(res_dup.is_ok(), "Idempotency should return OK");

    // --- ÉTAPE 3 : UPDATE LOCALE (v1 -> v2) ---
    let update_command_id = uuid::Uuid::now_v7().to_string();
    let account_id =
        Uuid::parse_str(&real_account_id).expect("Invalid account_id UUID returned by gRPC");
    let current_version: i64 = sqlx::query_scalar(
        "SELECT version FROM account_identity WHERE account_id = $1 AND region = $2",
    )
    .bind(account_id)
    .bind(region)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Failed to fetch current version for optimistic locking");

    let target = AccountTarget {
        account_id: real_account_id,
        region: region.to_string(),
        expected_version: current_version as u64,
    };

    let update_payload = UpdateLocaleRequest {
        command_id: update_command_id.clone(),
        target: Some(target),
        locale: "en-US".to_string(),
    };

    let upd_res = personal_client
        .update_locale(with_auth(
            update_payload,
            &auth_response.token.as_str(),
            region,
        ))
        .await;

    assert!(upd_res.is_ok(), "Update failed: {:?}", upd_res.err());

    // --- ÉTAPE 4 : VÉRIFICATIONS DB FINALES ---

    // 1. Vérification État & Version
    let row: (String, i64) = sqlx::query_as(
        "SELECT locale, version FROM account_identity WHERE account_id = $1 AND region = $2",
    )
    .bind(account_id)
    .bind(region)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Account not found in DB");

    assert_eq!(row.0, "en-US");
    assert_eq!(
        row.1, 2,
        "Version should be 2 after creation (v1) and update (v2)"
    );

    // 2. Vérification Idempotence (On vérifie les deux commandes distinctes)
    for cid in &[register_command_id, update_command_id] {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM processed_commands WHERE command_id = $1")
                .bind(uuid::Uuid::parse_str(cid).unwrap())
                .fetch_one(&ctx.pg_pool())
                .await
                .unwrap();
        assert_eq!(
            count.0, 1,
            "Command {} should be recorded exactly once",
            cid
        );
    }

    ctx.shutdown().await;
    Ok(())
}
