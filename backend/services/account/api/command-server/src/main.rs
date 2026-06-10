// backend/services/account/api/command-server/src/main.rs

use account::{
    AccountServiceBuilder,
    services::{
        AccountAccessService, AccountModerationService, AccountPersonalService,
        AccountRegistrationService, AccountSettingsService,
    },
};
use auth::{
    KeycloakValidator,
    interceptors::{AuthInterceptor, RegistrationInterceptor},
};
use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_sqlx::PostgresContext;
use std::{sync::Arc, time::Duration};
use tonic::transport::Server;

use shared_proto::account::v1::{
    account_access_service_server::AccountAccessServiceServer,
    account_moderation_service_server::AccountModerationServiceServer,
    account_personal_service_server::AccountPersonalServiceServer,
    account_registration_service_server::AccountRegistrationServiceServer,
    account_settings_service_server::AccountSettingsServiceServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let global_database_url =
        std::env::var("GLOBAL_DATABASE_URL").expect("GLOBAL_DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "account-service".to_string());

    let otp_ttl_secs = std::env::var("OTP_TTL_SECONDS")
        .unwrap_or_else(|_| "900".to_string()) // 15 minutes par défaut (900s)
        .parse::<u64>()
        .expect("OTP_TTL_SECONDS must be a valid u64");
    let otp_ttl = Duration::from_secs(otp_ttl_secs);

    let pg_ctx = PostgresContext::builder()?
        .with_url(database_url)
        .with_max_connections(20)
        .build()
        .await?;

    let global_pg_ctx = PostgresContext::builder()?
        .with_url(global_database_url)
        .with_max_connections(10)
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let builder = AccountServiceBuilder::new(
        pg_ctx.pool(),
        global_pg_ctx.pool(),
        redis_ctx.cache_repository(),
        otp_ttl,
    );
    let app_ctx = builder.build_context();
    let bus = builder.build_command_bus();

    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());
    let registration_interceptor = RegistrationInterceptor::new(validator);

    let registration_svc = AccountRegistrationService::new(bus.clone(), app_ctx.clone());
    let access_svc = AccountAccessService::new(bus.clone(), app_ctx.clone());
    let moderation_svc = AccountModerationService::new(bus.clone(), app_ctx.clone());
    let personal_svc = AccountPersonalService::new(bus.clone(), app_ctx.clone());
    let settings_svc = AccountSettingsService::new(bus, app_ctx);

    tracing::info!("🚀 Account Command Service listening on {}", addr);

    Server::builder()
        .add_service(AccountRegistrationServiceServer::with_interceptor(
            registration_svc,
            registration_interceptor,
        ))
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
