// backend/services/post/api/command-server/src/main.rs

use auth::{AuthInterceptor, KeycloakValidator};
use dotenvy::dotenv;
use infra_fred::{RedisContext, RedisIdempotencyRepository};
use infra_scylla::ScyllaContext;
use post::{
    PostServiceBuilder,
    resolvers_impl::{CachedProfileResolver, GrpcProfileSource},
    services::PostService,
};
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

use shared_proto::{
    post::v1::post_service_server::PostServiceServer,
    profile::v1::profile_query_service_client::ProfileQueryServiceClient,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ =
        fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("info,infra_scylla=debug,post=debug,tonic=debug")
            }))
            .try_init();

    dotenv().ok();

    let scylla_nodes_str =
        std::env::var("POST_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let keyspace_name =
        std::env::var("POST_SCYLLA_KEYSPACE").unwrap_or_else(|_| "posts".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let profile_service_url =
        std::env::var("PROFILE_SERVICE_URL").expect("PROFILE_SERVICE_URL must be set");

    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    let scylla_ctx = ScyllaContext::builder()?
        .with_nodes(scylla_nodes)
        .with_keyspace(keyspace_name) // Keyspace configuré dans ta migration CQL
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let redis_cache_repo = redis_ctx.repository();
    let redis_pool = redis_cache_repo.pool().clone();
    let idempotency_repo = Arc::new(RedisIdempotencyRepository::new(
        redis_pool.clone(),
        "post",
        7200,
    ));

    let grpc_channel = tonic::transport::Channel::from_shared(profile_service_url)?
        .connect()
        .await?;
    let grpc_client = ProfileQueryServiceClient::new(grpc_channel);

    let fallback_source = Arc::new(GrpcProfileSource::new(
        grpc_client,
        "main-region".to_string(),
    ));
    let profile_resolver = Arc::new(CachedProfileResolver::new(
        redis_cache_repo.clone(),
        fallback_source,
    ));

    let builder = PostServiceBuilder::new(
        scylla_ctx.keyspace(),
        scylla_ctx.session().clone(),
        redis_cache_repo.clone(),
        idempotency_repo,
        profile_resolver,
    );

    let app_ctx = builder.build_context().await?;
    let bus = builder.build_command_bus();

    let port = std::env::var("PORT").unwrap_or_else(|_| "50054".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm)
            .await
            .expect("Failed to initialize Keycloak validator pour le service Post"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());
    let post_svc = PostService::new(bus, app_ctx);

    tracing::info!(
        "🚀 Post Service (CQRS & Architecture Shardée de Production) listening on {}",
        addr
    );

    Server::builder()
        .add_service(PostServiceServer::with_interceptor(
            post_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
