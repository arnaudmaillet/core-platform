use auth::{AuthInterceptor, KeycloakValidator};
use dotenvy::dotenv;
use std::sync::Arc;
use tonic::transport::Server;

// 1. Imports du Shared Kernel (Socle technique)
use shared_kernel::application::{BaseAppContext, CommandBus};
use shared_kernel::infrastructure::postgres::factories::PostgresContext;
use shared_kernel::infrastructure::postgres::repositories::{
    PostgresIdempotencyRepository, PostgresOutboxRepository,
};
use shared_kernel::infrastructure::redis::factories::RedisContext;

// 2. Imports de la crate Account (Logique métier et Repositories)
use account::application::context::{AccountAppContext, AccountContext};
use account::infrastructure::postgres::repositories::PostgresAccountRepository;

// --- Use Cases : Access Management ---
use account::application::use_cases::access_management::link_sub_identity::{
    LinkSubIdentityCommand, LinkSubIdentityHandler,
};
use account::application::use_cases::access_management::register::{
    RegisterCommand, RegisterHandler,
};

// --- Use Cases : Lifecycle ---
use account::application::use_cases::lifecycle::activate::{ActivateCommand, ActivateHandler};
use account::application::use_cases::lifecycle::change_role::{
    ChangeRoleCommand, ChangeRoleHandler,
};
use account::application::use_cases::lifecycle::deactivate::{
    DeactivateCommand, DeactivateHandler,
};
use account::application::use_cases::lifecycle::suspend::{SuspendCommand, SuspendHandler};
use account::application::use_cases::lifecycle::unsuspend::{UnsuspendCommand, UnsuspendHandler};

// --- Use Cases : Moderation ---
use account::application::use_cases::moderation::ban::{BanCommand, BanHandler};
use account::application::use_cases::moderation::decrease_trust_score::{
    DecreaseTrustScoreCommand, DecreaseTrustScoreHandler,
};
use account::application::use_cases::moderation::increase_trust_score::{
    IncreaseTrustScoreCommand, IncreaseTrustScoreHandler,
};
use account::application::use_cases::moderation::lift_shadowban::{
    LiftShadowbanCommand, LiftShadowbanHandler,
};
use account::application::use_cases::moderation::shadowban::{ShadowbanCommand, ShadowbanHandler};
use account::application::use_cases::moderation::unban::{UnbanCommand, UnbanHandler};

// --- Use Cases : Settings ---
use account::application::use_cases::settings::add_push_token::{
    AddPushTokenCommand, AddPushTokenHandler,
};
use account::application::use_cases::settings::change_birth_date::{
    ChangeBirthDateCommand, ChangeBirthDateHandler,
};
use account::application::use_cases::settings::change_email::{
    ChangeEmailCommand, ChangeEmailHandler,
};
use account::application::use_cases::settings::change_phone_number::{
    ChangePhoneNumberCommand, ChangePhoneNumberHandler,
};
use account::application::use_cases::settings::change_region::{
    ChangeRegionCommand, ChangeRegionHandler,
};
use account::application::use_cases::settings::remove_push_token::{
    RemovePushTokenCommand, RemovePushTokenHandler,
};
use account::application::use_cases::settings::update_locale::{
    UpdateLocaleCommand, UpdateLocaleHandler,
};
use account::application::use_cases::settings::update_preferences::{
    UpdatePreferencesCommand, UpdatePreferencesHandler,
};
use account::application::use_cases::settings::update_timezone::{
    UpdateTimezoneCommand, UpdateTimezoneHandler,
};

// 3. Imports des Services gRPC (Interface)
use account::infrastructure::api::grpc::{
    GrpcAccessService, GrpcModerationService, GrpcPersonalService, GrpcSettingsService,
};

// 4. Serveurs générés par Tonic
use shared_proto::account::v1::account_access_service_server::AccountAccessServiceServer;
use shared_proto::account::v1::account_moderation_service_server::AccountModerationServiceServer;
use shared_proto::account::v1::account_personal_service_server::AccountPersonalServiceServer;
use shared_proto::account::v1::account_settings_service_server::AccountSettingsServiceServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialisation des variables d'environnement et du logging
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

    let pool = pg_ctx.pool().clone();

    // --- Instanciation des Repositories ---
    let account_repo = Arc::new(PostgresAccountRepository::new(
        pool.clone(),
        redis_ctx.repository(),
    ));
    let outbox_repo: Arc<PostgresOutboxRepository> =
        Arc::new(PostgresOutboxRepository::new(pool.clone()));
    let idempotency_repo = Arc::new(PostgresIdempotencyRepository::new("account_idempotency"));

    // --- Initialisation des Contextes ---
    let app_ctx = Arc::new(AccountAppContext::new(
        BaseAppContext::new(Some(pool.clone()), redis_ctx.repository().clone()),
        account_repo,
        outbox_repo,
        idempotency_repo,
    ));

    // --- Configuration du CommandBus ---
    let bus = Arc::new(configure_command_bus());

    // --- Démarrage du Serveur gRPC ---
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;
    let validator = Arc::new(
        KeycloakValidator::new(&keycloak_url, &keycloak_realm)
            .await
            .expect("Failed to initialize Keycloak validator"),
    );

    let auth_interceptor: AuthInterceptor = AuthInterceptor::new(validator.clone());

    // Ici, bus.clone() renverra bien Arc<CommandBus> aux services
    let access_svc = GrpcAccessService::new(bus.clone(), app_ctx.clone());
    let moderation_svc = GrpcModerationService::new(bus.clone(), app_ctx.clone());
    let personal_svc = GrpcPersonalService::new(bus.clone(), app_ctx.clone());
    let settings_svc = GrpcSettingsService::new(bus.clone(), app_ctx.clone());

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

fn configure_command_bus() -> CommandBus {
    let mut bus = CommandBus::new();

    // --- Access Management ---
    bus.register::<AccountContext, RegisterCommand, RegisterHandler>(RegisterHandler);
    bus.register::<AccountContext, LinkSubIdentityCommand, LinkSubIdentityHandler>(
        LinkSubIdentityHandler,
    );

    // --- Lifecycle ---
    bus.register::<AccountContext, ActivateCommand, ActivateHandler>(ActivateHandler);
    bus.register::<AccountContext, DeactivateCommand, DeactivateHandler>(DeactivateHandler);
    bus.register::<AccountContext, ChangeRoleCommand, ChangeRoleHandler>(ChangeRoleHandler);
    bus.register::<AccountContext, SuspendCommand, SuspendHandler>(SuspendHandler);
    bus.register::<AccountContext, UnsuspendCommand, UnsuspendHandler>(UnsuspendHandler);

    // --- Moderation ---
    bus.register::<AccountContext, BanCommand, BanHandler>(BanHandler);
    bus.register::<AccountContext, UnbanCommand, UnbanHandler>(UnbanHandler);
    bus.register::<AccountContext, ShadowbanCommand, ShadowbanHandler>(ShadowbanHandler);
    bus.register::<AccountContext, LiftShadowbanCommand, LiftShadowbanHandler>(
        LiftShadowbanHandler,
    );
    bus.register::<AccountContext, IncreaseTrustScoreCommand, IncreaseTrustScoreHandler>(
        IncreaseTrustScoreHandler,
    );
    bus.register::<AccountContext, DecreaseTrustScoreCommand, DecreaseTrustScoreHandler>(
        DecreaseTrustScoreHandler,
    );

    // --- Settings ---
    bus.register::<AccountContext, AddPushTokenCommand, AddPushTokenHandler>(AddPushTokenHandler);
    bus.register::<AccountContext, RemovePushTokenCommand, RemovePushTokenHandler>(
        RemovePushTokenHandler,
    );
    bus.register::<AccountContext, ChangeEmailCommand, ChangeEmailHandler>(ChangeEmailHandler);
    bus.register::<AccountContext, ChangePhoneNumberCommand, ChangePhoneNumberHandler>(
        ChangePhoneNumberHandler,
    );
    bus.register::<AccountContext, ChangeBirthDateCommand, ChangeBirthDateHandler>(
        ChangeBirthDateHandler,
    );
    bus.register::<AccountContext, ChangeRegionCommand, ChangeRegionHandler>(ChangeRegionHandler);
    bus.register::<AccountContext, UpdateLocaleCommand, UpdateLocaleHandler>(UpdateLocaleHandler);
    bus.register::<AccountContext, UpdateTimezoneCommand, UpdateTimezoneHandler>(
        UpdateTimezoneHandler,
    );
    bus.register::<AccountContext, UpdatePreferencesCommand, UpdatePreferencesHandler>(
        UpdatePreferencesHandler,
    );

    bus
}
