// backend/services/profile/api/command-server/tests/e2e_it.rs

use auth::{AuthInterceptor, KeycloakTestContext, KeycloakValidator, TokenValidator};
use profile::ProfileServiceBuilder;
use profile::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};
use profile::test_utils::ProfileTestContext;
use shared_kernel::core::{Identifier, Result};
use shared_kernel::types::{AccountId, ProfileId};
use shared_proto::profile::v1::profile_identity_service_client::ProfileIdentityServiceClient;
use shared_proto::profile::v1::profile_identity_service_server::ProfileIdentityServiceServer;
use shared_proto::profile::v1::profile_media_service_server::ProfileMediaServiceServer;
use shared_proto::profile::v1::profile_metadata_service_server::ProfileMetadataServiceServer;
use shared_proto::profile::v1::{ChangeHandleRequest, ProfileTarget};
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, metadata::MetadataValue};

// Helper Auth
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
    // 1. SETUP INFRA
    let ctx = ProfileTestContext::builder()
        .with_server(|pg_pool, redis_repo, _kafka, addr, shutdown_rx, ready_tx| {
            // 💡 Ajout de ready_tx
            async move {
                let auth_ctx = KeycloakTestContext::restore("master").await;
                let validator = Arc::new(
                    KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                        .await
                        .unwrap(),
                );
                let interceptor = AuthInterceptor::new(validator);

                let builder = ProfileServiceBuilder::new(pg_pool, redis_repo);
                let app_ctx = builder.build_context();
                let bus = builder.build_command_bus();

                let identity_svc = ProfileIdentityService::new(bus.clone(), app_ctx.clone());
                let media_svc = ProfileMediaService::new(bus.clone(), app_ctx.clone());
                let metadata_svc = ProfileMetadataService::new(bus, app_ctx);

                ready_tx.send(()).ok();

                Server::builder()
                    .add_service(ProfileIdentityServiceServer::with_interceptor(
                        identity_svc,
                        interceptor.clone(),
                    ))
                    .add_service(ProfileMediaServiceServer::with_interceptor(
                        media_svc,
                        interceptor.clone(),
                    ))
                    .add_service(ProfileMetadataServiceServer::with_interceptor(
                        metadata_svc,
                        interceptor,
                    ))
                    .serve_with_shutdown(addr, async {
                        shutdown_rx.await.ok();
                    })
                    .await
                    .unwrap();
            }
        })
        .build_e2e()
        .await;

    let mut identity_client = ProfileIdentityServiceClient::connect(ctx.kernel().grpc_url())
        .await
        .unwrap();

    // 2. AUTH & IDENTITY EXTRACTION
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let _ = auth_ctx
        .validator
        .validate(&auth_response.token)
        .expect("Token must be valid");

    let region_str = "EU";
    let region = shared_kernel::types::Region::try_new(region_str)?;

    let real_profile_id = ProfileId::generate(region);
    let real_account_id = AccountId::generate(region);

    let profile_uuid = real_profile_id.as_uuid();
    let account_uuid = real_account_id.uuid();

    // 3. PRÉPARATION : Simulation d'un profil existant (v0) avec des Smart IDs conformes
    sqlx::query(
        "INSERT INTO user_profiles (profile_id, account_id, region, handle, display_name, version, is_private, created_at, updated_at) 
         VALUES ($1, $2, $3, $4, $5, 0, false, NOW(), NOW())"
    )
    .bind(profile_uuid)     // $1
    .bind(account_uuid)     // $2
    .bind(region_str)       // $3
    .bind("alice_rocks")    // $4
    .bind("Alice")          // $5
    .execute(&ctx.pg_pool())
    .await
    .unwrap();

    // 4. ACT : Changement de Handle (v0 -> v1)
    let command_id = uuid::Uuid::now_v7().to_string();
    let target = ProfileTarget {
        // 💡 FIX : On passe la version String du vrai ProfileId contenant la région
        profile_id: real_profile_id.to_string(),
        region: region_str.to_string(),
        expected_version: 0,
    };

    let change_handle_req = ChangeHandleRequest {
        command_id: command_id.clone(),
        target: Some(target),
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

    // 6. VÉRIFICATIONS FINALES EN DB
    let row: (String, i64) = sqlx::query_as(
        "SELECT handle, version FROM user_profiles WHERE profile_id = $1 AND region = $2",
    )
    .bind(profile_uuid)
    .bind(region_str)
    .fetch_one(&ctx.pg_pool())
    .await
    .expect("Profile should exist in DB");

    assert_eq!(row.0, "alice_wonderland");
    assert_eq!(row.1, 1, "Version should be 1 after one update");

    // Vérification de l'idempotence (processed_commands)
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM processed_commands WHERE command_id = $1")
            .bind(uuid::Uuid::parse_str(&command_id).unwrap())
            .fetch_one(&ctx.pg_pool())
            .await
            .unwrap();

    assert_eq!(count.0, 1, "Command should be recorded once");

    ctx.shutdown().await;
    Ok(())
}
