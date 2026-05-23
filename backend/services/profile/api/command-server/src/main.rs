// backend/services/profile/api/command-server/src/main.rs

use auth::{AuthInterceptor, KeycloakValidator};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_sqlx::PostgresContext;
use std::sync::Arc;
use tonic::transport::Server;
use tracing_subscriber::{EnvFilter, fmt};

use profile::ProfileServiceBuilder;
use profile::services::{ProfileIdentityService, ProfileMediaService, ProfileMetadataService};

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
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=debug,account=debug,tonic=debug")),
        )
        .with_test_writer()
        .try_init();

    // 1. Initialisation
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // 2. Configuration de l'Infrastructure
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");

    // Initialisation des contextes (Postgres + Redis)
    let pg_ctx = PostgresContext::builder()?
        .with_url(database_url)
        .with_max_connections(20)
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 3. Assemblage du Domaine via le Builder
    // On passe le pool PG et le repo Redis
    let builder = ProfileServiceBuilder::new(pg_ctx.pool(), redis_ctx.repository());
    let app_ctx = builder.build_context();
    let bus = builder.build_command_bus();

    // 4. Configuration gRPC & Auth
    let port = std::env::var("PORT").unwrap_or_else(|_| "50052".to_string()); // Port différent d'Account
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());

    // 5. Instanciation du Service gRPC
    // On injecte le bus de commande et le contexte d'application
    let identity_svc = ProfileIdentityService::new(bus.clone(), app_ctx.clone());
    let media_svc = ProfileMediaService::new(bus.clone(), app_ctx.clone());
    let metadata_svc = ProfileMetadataService::new(bus.clone(), app_ctx.clone());

    tracing::info!("🚀 Profile Service listening on {}", addr);

    // 6. Lancement du Serveur
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
            auth_interceptor.clone(),
        ))
        .serve(addr)
        .await?;

    Ok(())
}
