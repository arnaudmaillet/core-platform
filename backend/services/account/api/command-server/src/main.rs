// backend/services/account/api/command-server/src/main.rs

use account::db::{PostgresAccountRepository, PostgresGlobalIdentityRegistry};
use account::fred::FredOtpRepository;
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
use infra_sqlx::{
    PostgresContext, PostgresIdempotencyRepository, PostgresOutboxRepository,
    PostgresTransactionManager,
};
use shared_kernel::command::CommandBus;
use shared_kernel::environment::ClusterContext;
use shared_kernel::types::Region;
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

    // 1. Chargement et validation des variables d'environnement
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let global_database_url =
        std::env::var("GLOBAL_DATABASE_URL").expect("GLOBAL_DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    let keycloak_realm = std::env::var("KEYCLOAK_REALM").expect("KEYCLOAK_REALM must be set");
    let keycloak_audience =
        std::env::var("KEYCLOAK_AUDIENCE").unwrap_or_else(|_| "account-service".to_string());

    let region_raw = std::env::var("REGION").unwrap_or_else(|_| "eu-west-1".to_string());
    let region = Region::try_from(region_raw.as_str())?;
    let cluster_ctx = ClusterContext::new(region);

    let otp_ttl_secs = std::env::var("OTP_TTL_SECONDS")
        .unwrap_or_else(|_| "900".to_string()) // 15 minutes par défaut (900s)
        .parse::<u64>()
        .expect("OTP_TTL_SECONDS must be a valid u64");
    let otp_ttl = Duration::from_secs(otp_ttl_secs);

    // 2. Initialisation des Contextes d'Infrastructure Drivers
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

    // 3. Instanciation des Adaptateurs (Repositories Concrets)
    let pool = pg_ctx.pool();
    let global_pool = global_pg_ctx.pool();

    let account_repo = Arc::new(PostgresAccountRepository::new(pool.clone()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));
    let cache_repo = redis_ctx.cache_repository();
    let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new_with_pool(
        pool.clone(),
        "account",
    ));
    let global_registry = Arc::new(PostgresGlobalIdentityRegistry::new(global_pool));
    let otp_repo = Arc::new(FredOtpRepository::new(cache_repo.clone(), otp_ttl));
    let tx_manager = Arc::new(PostgresTransactionManager::new(pool));

    // 4. Assemblage via le Builder de Service et configuration du Kernel
    let service = AccountServiceBuilder::new(
        account_repo,
        outbox_repo,
        idempotency_repo.clone(),
        global_registry,
        otp_repo,
        tx_manager,
        cluster_ctx,
    );

    let kernel = service.build_kernel_ctx();

    // 5. Initialisation et configuration du CommandBus avec ses Gardes Idempotents
    let mut command_bus = CommandBus::new(Some(idempotency_repo), Some(cache_repo));

    service.register_handlers(&mut command_bus);
    let bus = Arc::new(command_bus);

    // 6. Configuration de la couche de transport gRPC Server & Interceptors
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm, keycloak_audience)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor = AuthInterceptor::new(validator.clone());
    let registration_interceptor = RegistrationInterceptor::new(validator);

    let registration_svc = AccountRegistrationService::new(bus.clone(), kernel.clone());
    let access_svc = AccountAccessService::new(bus.clone(), kernel.clone());
    let moderation_svc = AccountModerationService::new(bus.clone(), kernel.clone());
    let personal_svc = AccountPersonalService::new(bus.clone(), kernel.clone());
    let settings_svc = AccountSettingsService::new(bus, kernel);

    tracing::info!(
        "🚀 Account Command Service Shard [{}] listening on {}",
        region,
        addr
    );

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
