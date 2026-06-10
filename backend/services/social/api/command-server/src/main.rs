// backend/services/social/api/command-server/src/main.rs

use auth::{KeycloakValidator, interceptors::AuthInterceptor};
use dotenvy::dotenv;
use infra_fred::{RedisContext, RedisIdempotencyRepository};
use infra_scylla::ScyllaContext;
use social::SocialServiceBuilder;
use social::services::SocialService;
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

use shared_proto::social::v1::social_service_server::SocialServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=debug,account=debug,tonic=debug")),
        )
        .with_test_writer()
        .try_init();

    dotenv().ok();
    tracing_subscriber::fmt::init();

    let scylla_nodes_str =
        std::env::var("SOCIAL_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "social-service".to_string());

    // 3. Initialisation des contextes technologiques
    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let scylla_ctx = ScyllaContext::builder_from_env()?
        .with_nodes(scylla_nodes)
        .with_keyspace("social_network")
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let redis_cache_repo = redis_ctx.cache_repository();
    let redis_pool = redis_cache_repo.pool().clone();

    let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
        redis_pool.clone(),
        "social",
        7200,
    ));

    let builder = SocialServiceBuilder::new(
        scylla_ctx.session(),
        redis_pool,
        redis_cache_repo,
        idempotency_repo,
    );

    let app_ctx = builder.build_context().await;
    let bus = builder.build_command_bus();

    let port = std::env::var("PORT").unwrap_or_else(|_| "50053".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());
    let social_svc = SocialService::new(bus, app_ctx);

    tracing::info!(
        "🚀 Social Service (Hyperscale Graphes & Compteurs de Production) listening on {}",
        addr
    );

    Server::builder()
        .add_service(SocialServiceServer::with_interceptor(
            social_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
