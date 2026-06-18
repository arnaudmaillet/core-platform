use auth::{KeycloakValidator, interceptors::AuthInterceptor};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_scylla::ScyllaContext;
use post_assembly::PostQueryAssembly;
use post_proto_bridge::v1::post_query_service_server::PostQueryServiceServer;
use post_query_server::PostQueryService;
use shared_kernel::types::Region;
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

    // 1. Chargement et validation des variables d'environnement de Sharding Régional
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION"))
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;

    let scylla_nodes_str =
        std::env::var("POST_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "post-service".to_string());

    // 2. Initialisation des Contextes d'Infrastructure Drivers
    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let keyspace_name = format!("{}_post_storage", local_region.to_string().to_lowercase());

    // Drivers Redis (Crucial pour le Read Model / Cache-Aside de lecture)
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let cache_repo = redis_ctx.cache_repository();

    // Drivers ScyllaDB (Peut pointer vers des réplicas de lecture en prod)
    let scylla_ctx: ScyllaContext = ScyllaContext::builder_from_env()?
        .with_nodes(scylla_nodes)
        .with_keyspace(&keyspace_name)
        .build()
        .await?;

    // 3. Utilisation du Bootstrap de lecture exclusif (Query)
    // Remarque : Pas de CommandBus ici, le serveur de lecture n'en a pas besoin.
    let container = PostQueryAssembly::bootstrap(
        scylla_ctx.session().clone(),
        cache_repo.clone(),
        keyspace_name,
    )
    .await?;

    // Instanciation de ton service de lecture gRPC
    let post_query_svc = PostQueryService::new(container);

    // 4. Configuration réseau de l'exposition du serveur gRPC de Lecture
    // Pense à attribuer un port différent du command-server en local (ex: 50055)
    let port = std::env::var("PORT").unwrap_or_else(|_| "50055".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator pour le service Post Query"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    tracing::info!(
        "🚀 Post Query Service Shard [{:?}] listening on {}",
        local_region,
        addr
    );

    // 5. Lancement du Serveur gRPC de Lecture
    Server::builder()
        .add_service(PostQueryServiceServer::with_interceptor(
            post_query_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
