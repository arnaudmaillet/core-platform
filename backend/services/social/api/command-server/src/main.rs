// backend/services/social/api/command-server/src/main.rs

use auth::{AuthInterceptor, KeycloakValidator};
use dotenvy::dotenv;
use infra_fred::{RedisContext, RedisIdempotencyRepository};
use infra_scylla::ScyllaContext;
use social::SocialServiceBuilder;
use social::services::SocialService;
use std::sync::Arc;
use tonic::transport::Server;

use shared_proto::social::v1::social_service_server::SocialServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialisation des variables d'environnement et du traçage
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // 2. Récupération de la configuration d'infrastructure
    let scylla_nodes_str =
        std::env::var("SOCIAL_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");

    // 3. Initialisation des contextes technologiques
    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let scylla_ctx = ScyllaContext::builder_raw()
        .with_nodes(scylla_nodes)
        .with_keyspace("social_network")
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 4. Extraction des composants Redis et instanciation de TON dépôt d'idempotence
    let redis_cache_repo = redis_ctx.repository();
    let redis_pool = redis_cache_repo.pool().clone();

    // Connexion à TON dépôt générique. On met un TTL de 2 heures (7200 secondes).
    let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
        redis_pool.clone(),
        "social",
        7200,
    ));

    // 5. Alignement du Builder de domaine
    let builder = SocialServiceBuilder::new(
        scylla_ctx.session(),
        redis_pool,
        redis_cache_repo,
        idempotency_repo,
    );

    let app_ctx = builder.build_context().await;
    let bus = builder.build_command_bus();

    // 6. Configuration réseau gRPC & Sécurité Auth (Keycloak)
    let port = std::env::var("PORT").unwrap_or_else(|_| "50053".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    // 7. Instanciation de l'implémentation du service gRPC Tonic
    let social_svc = SocialService::new(bus, app_ctx);

    tracing::info!(
        "🚀 Social Service (Hyperscale Graphes & Compteurs de Production) listening on {}",
        addr
    );

    // 8. Lancement du serveur d'écoute gRPC
    Server::builder()
        .add_service(SocialServiceServer::with_interceptor(
            social_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
