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
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
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

    // 1. Chargement et validation des variables d'environnement de Sharding
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION")) // Fallback pour uniformiser avec Account
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;
    let cluster_ctx = ClusterContext::new(local_region);

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "profile-service".to_string());

    // 2. Initialisation des Contextes d'Infrastructure Drivers
    let scylla_ctx = ScyllaContext::builder_from_env()?.build().await?;
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 3. Instanciation des Adaptateurs (Repositories Concrets)
    let session = scylla_ctx.session().clone();
    let cache_repo = redis_ctx.cache_repository();
    let idempotency_repo = redis_ctx.idempotency_repository();

    let routing_store = Arc::new(
        ScyllaProfileRoutingStore::new(session.clone())
            .await
            .expect("Failed to prepare ScyllaDB global routing statements"),
    );

    // 🛠️ Calcul déterministe du nom du keyspace régional pour ce pod d'écriture
    let keyspace_name = format!(
        "{}_profile_storage",
        local_region.to_string().to_lowercase()
    );

    // 🛠️ Passage de la String du keyspace conformément au contrat découplé
    let profile_store = Arc::new(
        ScyllaProfileStore::new(session, keyspace_name)
            .await
            .expect("Failed to prepare ScyllaDB regional profile statements"),
    );

    // 4. Assemblage via le Builder de Service et configuration du Kernel
    let service = ProfileServiceBuilder::new(
        profile_store,
        routing_store,
        idempotency_repo.clone(),
        cluster_ctx,
    );

    let kernel = service.build_kernel_ctx();

    // 5. Initialisation et configuration du CommandBus avec ses Gardes Idempotents
    let mut command_bus = CommandBus::new(cache_repo, idempotency_repo);
    service.register_handlers(&mut command_bus);
    let bus = Arc::new(command_bus);

    // 6. Configuration de la couche de transport gRPC Server & Interceptors
    let port = std::env::var("PORT").unwrap_or_else(|_| "50052".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    let identity_svc = ProfileIdentityService::new(bus.clone(), kernel.clone());
    let media_svc = ProfileMediaService::new(bus.clone(), kernel.clone());
    let metadata_svc = ProfileMetadataService::new(bus, kernel);

    tracing::info!(
        "🚀 Profile Command Service Shard [{:?}] listening on {}",
        local_region,
        addr
    );

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
