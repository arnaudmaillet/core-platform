// backend/services/profile/api/command-server/src/main.rs

use auth::{KeycloakValidator, interceptors::AuthInterceptor};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_scylla::ScyllaContext;
use profile::stores::{ScyllaProfileRoutingStore, ScyllaProfileStore};
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

use profile::ProfileServiceBuilder;
use profile::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};
use shared_kernel::types::Region;

use shared_proto::profile::v1::{
    profile_identity_service_server::ProfileIdentityServiceServer,
    profile_media_service_server::ProfileMediaServiceServer,
    profile_metadata_service_server::ProfileMetadataServiceServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,scylla=debug,account=debug,tonic=debug")),
        )
        .with_test_writer()
        .try_init();

    dotenv().ok();

    let region_str = std::env::var("CLUSTER_REGION").unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "profile-service".to_string());

    let scylla_ctx = ScyllaContext::builder_from_env()?.build().await?;
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    let routing_store = Arc::new(
        ScyllaProfileRoutingStore::new(scylla_ctx.session().clone())
            .await
            .expect("Failed to prepare ScyllaDB global routing statements"),
    );

    let profile_store = Arc::new(
        ScyllaProfileStore::new(scylla_ctx.session().clone(), local_region)
            .await
            .expect("Failed to prepare ScyllaDB regional profile statements"),
    );

    let builder = ProfileServiceBuilder::new(
        profile_store,
        routing_store,
        redis_ctx.cache_repository(),
        redis_ctx.idempotency_repository(),
        local_region,
    );

    let app_ctx = Arc::new(builder.build_context());
    let bus = builder.build_command_bus();

    let port = std::env::var("PORT").unwrap_or_else(|_| "50052".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    let identity_svc = ProfileIdentityService::new(bus.clone(), app_ctx.clone());
    let media_svc = ProfileMediaService::new(bus.clone(), app_ctx.clone());
    let metadata_svc = ProfileMetadataService::new(bus, app_ctx);

    tracing::info!(
        "🚀 Profile Service ({:?}) listening on {}",
        local_region,
        addr
    );

    // 4. Lancement du Serveur gRPC
    Server::builder()
        .add_service(ProfileIdentityServiceServer::with_interceptor(
            identity_svc,
            auth_interceptor.clone(),
        ))
        .add_service(ProfileMediaServiceServer::with_interceptor(
            media_svc,
            auth_interceptor.clone(),
        ))
        .add_service(ProfileMetadataServiceServer::with_interceptor(
            metadata_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
