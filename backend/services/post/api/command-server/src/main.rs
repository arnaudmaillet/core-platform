// backend/services/post/api/command-server/src/main.rs

use auth::{KeycloakValidator, interceptors::AuthInterceptor};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_scylla::ScyllaContext;
use post::{
    PostServiceBuilder,
    repositories_impl::ScyllaPostStore,
    resolvers_impl::{CachedProfileResolver, GrpcProfileSource},
    services::PostService,
};
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

use shared_kernel::types::Region;
use shared_kernel::{command::CommandBus, environment::ClusterContext};

use shared_proto::{
    post::v1::post_service_server::PostServiceServer,
    profile::v1::profile_query_service_client::ProfileQueryServiceClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,scylla=debug,post=debug,tonic=debug")),
        )
        .try_init();

    dotenv().ok();

    // 1. Chargement et validation des variables d'environnement de Sharding Régional
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION")) 
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;
    let cluster_ctx = ClusterContext::new(local_region);

    let scylla_nodes_str =
        std::env::var("POST_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "post-service".to_string());
    let profile_service_url =
        std::env::var("PROFILE_SERVICE_URL").expect("PROFILE_SERVICE_URL must be set");

    // 2. Initialisation des Contextes d'Infrastructure Drivers (ScyllaDB & Redis)
    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Calcul déterministe du Keyspace de persistance souverain pour les Posts de cette région
    let keyspace_name = format!("{}_post_storage", local_region.to_string().to_lowercase());

    let scylla_ctx: ScyllaContext = ScyllaContext::builder_from_env()?
        .with_nodes(scylla_nodes)
        .with_keyspace(&keyspace_name)
        .build()
        .await?;
    let session = scylla_ctx.session().clone();
    let post_repo = Arc::new(ScyllaPostStore::new(session.clone(), keyspace_name.as_str()).await?);
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let redis_cache_repo = redis_ctx.cache_repository();
    let idempotency_repo = redis_ctx.idempotency_repository();

    // 3. Initialisation du Client gRPC d'inter-service pour le Profile Resolver
    let grpc_channel = tonic::transport::Channel::from_shared(profile_service_url)?
        .connect()
        .await?;
    let grpc_client = ProfileQueryServiceClient::new(grpc_channel);

    // Injection de la région locale dans la source de fallback pour optimiser les requêtes cross-shards
    let fallback_source = Arc::new(GrpcProfileSource::new(
        grpc_client,
        local_region.to_string(),
    ));

    let profile_resolver = Arc::new(CachedProfileResolver::new(
        redis_cache_repo.clone(),
        fallback_source,
    ));

    // 4. Assemblage via le Builder de Service et configuration du Kernel
    let service = PostServiceBuilder::new(post_repo, profile_resolver, cluster_ctx);

    let kernel = service.build_kernel_ctx();
    let mut command_bus = CommandBus::new(redis_ctx.cache_repository(), idempotency_repo);

    service.register_handlers(&mut command_bus);

    // 5. Configuration réseau de l'exposition du serveur gRPC
    let port = std::env::var("PORT").unwrap_or_else(|_| "50054".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator pour le service Post"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());
    let post_svc = PostService::new(command_bus, kernel);

    tracing::info!(
        "🚀 Post Command Service Shard [{:?}] listening on {}",
        local_region,
        addr
    );

    // 6. Lancement du Serveur gRPC
    Server::builder()
        .add_service(PostServiceServer::with_interceptor(
            post_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
