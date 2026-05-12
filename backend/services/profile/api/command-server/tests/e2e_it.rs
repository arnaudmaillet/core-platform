// backend/services/profile/api/command-server/tests/e2e_it.rs

use auth::{AuthInterceptor, KeycloakTestContext, KeycloakValidator, TokenValidator};
use profile::ProfileServiceBuilder;
use profile::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};
use shared_kernel::domain::repositories::CacheRepository;
use shared_kernel::infrastructure::utils::{E2EServerStarter, InfrastructureKernelTestContext};
use shared_proto::profile::v1::profile_identity_service_client::ProfileIdentityServiceClient;
use shared_proto::profile::v1::profile_identity_service_server::ProfileIdentityServiceServer;
use shared_proto::profile::v1::profile_media_service_server::ProfileMediaServiceServer;
use shared_proto::profile::v1::profile_metadata_service_server::ProfileMetadataServiceServer;
use shared_proto::profile::v1::{ChangeHandleRequest, ProfileTarget};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::oneshot;
use tonic::async_trait;
use tonic::transport::Server;
use tonic::{Request, metadata::MetadataValue};

struct ProfileServerStarter;

#[async_trait]
impl E2EServerStarter for ProfileServerStarter {
    async fn start_server(
        &self,
        pg_pool: sqlx::PgPool,
        redis_repo: Arc<dyn CacheRepository>,
        addr: SocketAddr,
        shutdown_rx: oneshot::Receiver<()>,
    ) {
        // 1. Setup Auth (comme dans main.rs mais via Keycloak de test)
        let auth_ctx = KeycloakTestContext::restore("master").await;
        let validator = Arc::new(
            KeycloakValidator::new(&auth_ctx.uri, &auth_ctx.realm)
                .await
                .unwrap(),
        );
        let interceptor = AuthInterceptor::new(validator);

        // 2. Setup Domaine
        let builder = ProfileServiceBuilder::new(pg_pool, redis_repo);
        let app_ctx = builder.build_context();
        let bus = builder.build_command_bus();

        let identity_svc = ProfileIdentityService::new(bus.clone(), app_ctx.clone());
        let media_svc = ProfileMediaService::new(bus.clone(), app_ctx.clone());
        let metadata_svc = ProfileMetadataService::new(bus, app_ctx);

        // 3. Serveur gRPC
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
}

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
async fn test_e2e_complete_profile_lifecycle() -> shared_kernel::errors::Result<()> {
    // 1. SETUP INFRA
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let kernel_migs = manifest_dir.join("../../../../../crates/shared-kernel/migrations/postgres");
    let profile_migs = manifest_dir.join("../../../../../crates/profile/migrations/postgres");

    let ctx = InfrastructureKernelTestContext::builder()
        .with_postgres(&[
            kernel_migs.to_str().unwrap(),
            profile_migs.to_str().unwrap(),
        ])
        .with_redis()
        .with_server(ProfileServerStarter)
        .build_e2e()
        .await;

    let mut identity_client = ProfileIdentityServiceClient::connect(ctx.grpc_url())
        .await
        .unwrap();

    // 2. AUTH & IDENTITY EXTRACTION
    let auth_ctx = KeycloakTestContext::restore("master").await;
    let auth_response = auth_ctx.get_admin_token().await?;

    let claims = auth_ctx
        .validator
        .validate(&auth_response.token)
        .expect("Token must be valid");

    let sub_id_str = claims.sub_id.as_str();
    let sub_uuid = uuid::Uuid::parse_str(sub_id_str)
        .expect("Le sub_id de Keycloak doit être un UUID valide pour ce test");
    let true_sub_id = claims.sub_id.to_string(); // String pour le gRPC
    let region = "EU";

    // 3. PRÉPARATION : Simulation d'un profil existant (v0)
    // On simule l'arrivée d'un utilisateur qui vient de s'enregistrer
    sqlx::query(
        "INSERT INTO user_profiles (profile_id, account_id, region_code, handle, display_name, version, is_private, created_at, updated_at) 
         VALUES ($1, $1, $2, $3, $4, 0, false, NOW(), NOW())"
    )
    .bind(sub_uuid)
    .bind(region)
    .bind("alice_rocks")
    .bind("Alice")
    .execute(&ctx.postgres().pool())
    .await
    .unwrap();

    // 4. ACT : Changement de Handle (v0 -> v1)
    let command_id = uuid::Uuid::now_v7().to_string();
    let target = ProfileTarget {
        profile_id: true_sub_id.clone(),
        region: region.to_string(),
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
            region,
        ))
        .await;

    assert!(res.is_ok());

    // 6. VÉRIFICATIONS FINALES EN DB
    let row: (String, i64) = sqlx::query_as(
        "SELECT handle, version FROM user_profiles WHERE profile_id = $1 AND region_code = $2",
    )
    .bind(sub_uuid)
    .bind(region)
    .fetch_one(&ctx.postgres().pool())
    .await
    .expect("Profile should exist in DB");

    assert_eq!(row.0, "alice_wonderland");
    assert_eq!(row.1, 1, "Version should be 1 after one update");

    // Vérification de l'idempotence (processed_commands)
    let count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM processed_commands WHERE command_id = $1")
            .bind(uuid::Uuid::parse_str(&command_id).unwrap())
            .fetch_one(&ctx.postgres().pool())
            .await
            .unwrap();

    assert_eq!(count.0, 1, "Command should be recorded once");

    ctx.shutdown().await;
    Ok(())
}
