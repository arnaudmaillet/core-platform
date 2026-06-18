use auth::{KeycloakValidator, interceptors::AuthInterceptor};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_scylla::ScyllaContext;
use post_assembly::PostCommandAssembly;
use post_command_server::PostCommandService;
use post_proto_bridge::v1::post_command_service_server::PostCommandServiceServer;
use shared_kernel::types::Region;
use shared_kernel::{command::CommandBus, environment::ClusterContext};
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,scylla=debug,post=debug,tonic=debug")),
        )
        .try_init();

    dotenv().ok();

    // Chargement et validation des variables d'environnement de Sharding Régional
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION"))
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;
    let cluster_ctx = ClusterContext::new(local_region.clone());

    let scylla_nodes_str =
        std::env::var("POST_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "post-service".to_string());

    // Initialisation des Contextes d'Infrastructure Drivers (ScyllaDB & Redis)
    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let keyspace_name = format!("{}_post_storage", local_region.to_string().to_lowercase());

    // Drivers Redis
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let cache_repo = redis_ctx.cache_repository();

    // Drivers ScyllaDB
    let scylla_ctx: ScyllaContext = ScyllaContext::builder_from_env()?
        .with_nodes(scylla_nodes)
        .with_keyspace(&keyspace_name)
        .build()
        .await?;
    let command_bus = CommandBus::new(None, None);

    // 4. Utilisation du Bootstrap d'écriture exclusif (Command)
    let container = PostCommandAssembly::bootstrap(
        scylla_ctx.session().clone(),
        cache_repo.clone(),
        keyspace_name,
        cluster_ctx,
        command_bus,
    )
    .await?;

    let post_cmd_svc = PostCommandService::new(container);

    // Configuration réseau de l'exposition du serveur gRPC
    let port = std::env::var("PORT").unwrap_or_else(|_| "50054".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator pour le service Post"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    tracing::info!(
        "🚀 Post Command Service Shard [{:?}] listening on {}",
        local_region,
        addr
    );

    // 5. Lancement du Serveur gRPC avec le serveur spécifique généré
    Server::builder()
        .add_service(PostCommandServiceServer::with_interceptor(
            post_cmd_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
