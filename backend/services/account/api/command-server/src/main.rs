use account::{
    AccountServiceBuilder,
    services::{
        AccountAccessService, AccountModerationService, AccountPersonalService,
        AccountSettingsService,
    },
};
use auth::{AuthInterceptor, KeycloakValidator};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_sqlx::PostgresContext;
use std::sync::Arc;
use tonic::transport::Server;

use shared_proto::account::v1::{
    account_access_service_server::AccountAccessServiceServer,
    account_moderation_service_server::AccountModerationServiceServer,
    account_personal_service_server::AccountPersonalServiceServer,
    account_settings_service_server::AccountSettingsServiceServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // --- Configuration de l'Infrastructure ---
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");

    let pg_ctx = PostgresContext::builder()?
        .with_url(database_url)
        .with_max_connections(20)
        .build()
        .await?;
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    let builder = AccountServiceBuilder::new(pg_ctx.pool(), redis_ctx.repository());
    let app_ctx = builder.build_context();
    let bus = builder.build_command_bus();

    // --- Démarrage du Serveur gRPC ---
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor: AuthInterceptor = AuthInterceptor::new(validator.clone());

    let access_svc = AccountAccessService::new(bus.clone(), app_ctx.clone());
    let moderation_svc = AccountModerationService::new(bus.clone(), app_ctx.clone());
    let personal_svc = AccountPersonalService::new(bus.clone(), app_ctx.clone());
    let settings_svc = AccountSettingsService::new(bus.clone(), app_ctx.clone());

    tracing::info!("🚀 Account Command Service listening on {}", addr);

    Server::builder()
        .add_service(AccountAccessServiceServer::with_interceptor(
            access_svc,
            auth_interceptor.clone(),
        ))
        .add_service(AccountModerationServiceServer::with_interceptor(
            moderation_svc,
            auth_interceptor.clone(),
        ))
        .add_service(AccountPersonalServiceServer::with_interceptor(
            personal_svc,
            auth_interceptor.clone(),
        ))
        .add_service(AccountSettingsServiceServer::with_interceptor(
            settings_svc,
            auth_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
