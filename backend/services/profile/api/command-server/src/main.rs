// backend/services/profile/api/command-server/src/main.rs

use std::sync::Arc;
use tonic::transport::Server;
use profile::application::remove_avatar::RemoveAvatarUseCase;
use profile::application::remove_banner::RemoveBannerUseCase;
use profile::application::update_avatar::UpdateAvatarUseCase;
use profile::application::update_banner::UpdateBannerUseCase;
use profile::application::update_bio::UpdateBioUseCase;
use profile::application::update_display_name::UpdateDisplayNameUseCase;
use profile::application::update_location_label::UpdateLocationLabelUseCase;
use profile::application::update_privacy::UpdatePrivacyUseCase;
use profile::application::update_social_links::UpdateSocialLinksUseCase;
// Application
use profile::application::update_username::UpdateUsernameUseCase;

// Infrastructure - API
use profile::infrastructure::api::grpc::handlers::{IdentityHandler, MediaHandler, MetadataHandler};
use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_server::ProfileIdentityServiceServer;
use profile::infrastructure::api::grpc::profile_v1::profile_media_service_server::ProfileMediaServiceServer;
use profile::infrastructure::api::grpc::profile_v1::profile_metadata_service_server::ProfileMetadataServiceServer;
// Infrastructure - Repositories (Spécifiques au Profile)
use profile::infrastructure::postgres::utils::run_postgres_migrations;
use profile::infrastructure::scylla::utils::run_scylla_migrations;
use profile::infrastructure::postgres::repositories::PostgresProfileRepository;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::repositories::CompositeProfileRepository;

// Shared Kernel
use shared_kernel::infrastructure::grpc::region_interceptor;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::postgres::factories::{create_postgres_pool, DbConfig};
use shared_kernel::infrastructure::scylla::factories::{create_scylla_session, ScyllaConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());
    let addr = format!("0.0.0.0:{}", port).parse()?;

    // --- 1. INITIALISATION DES CLIENTS DB ---

    // Configuration et création de la pool Postgres (Shared Kernel)
    let mut db_config = DbConfig::from_env()?;
    db_config.max_connections = 5;
    let pool = create_postgres_pool(&db_config).await?;

    run_postgres_migrations(&pool).await?;

    // Configuration et création de la session ScyllaDB (Shared Kernel)
    let scylla_config = ScyllaConfig::from_env()?;
    let scylla_session = create_scylla_session(&scylla_config).await?;

    run_scylla_migrations(&scylla_session).await?;

    // --- 2. INITIALISATION DES REPOSITORIES (Infrastructure) ---

    // Implémentations techniques (On clone les pools car elles sont conçues pour ça)
    let identity_postgres = Arc::new(PostgresProfileRepository::new(pool.clone()));
    let stats_scylla = Arc::new(ScyllaProfileRepository::new(scylla_session.clone()));

    // L'orchestrateur (Façade Composite) qui masque la dualité DB au Domaine
    let profile_repo = Arc::new(CompositeProfileRepository::new(
        identity_postgres.clone(),
        stats_scylla.clone()
    ));

    // Outils techniques partagés pour la cohérence des données (Transaction + Outbox)
    let tx_manager = Arc::new(PostgresTransactionManager::new(pool.clone()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pool.clone()));

    // --- 3. INITIALISATION DES USE CASES (Application) ---

    let update_username_usecase = Arc::new(UpdateUsernameUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_display_name_use_case = Arc::new(UpdateDisplayNameUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_privacy_use_case = Arc::new(UpdatePrivacyUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_avatar_use_case = Arc::new(UpdateAvatarUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let remove_avatar_use_case = Arc::new(RemoveAvatarUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_banner_use_case = Arc::new(UpdateBannerUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let remove_banner_use_case = Arc::new(RemoveBannerUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let bio_update_use_case = Arc::new(UpdateBioUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_location_label_use_case = Arc::new(UpdateLocationLabelUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    let update_social_links_use_case = Arc::new(UpdateSocialLinksUseCase::new(
        profile_repo.clone(),
        outbox_repo.clone(),
        tx_manager.clone(),
    ));

    // --- 4. INITIALISATION DES HANDLERS (API) ---

    let identity_handler = IdentityHandler::new(update_username_usecase, update_display_name_use_case, update_privacy_use_case);
    let media_handler = MediaHandler::new(update_avatar_use_case, remove_avatar_use_case, update_banner_use_case, remove_banner_use_case);
    let metadata_handler = MetadataHandler::new(bio_update_use_case, update_location_label_use_case, update_social_links_use_case);

    // --- 5. DÉMARRAGE DU SERVEUR TONIC ---

    println!("🚀 Profile Query-Server listening on {}", addr);

    Server::builder()
        // Utilisation de l'intercepteur de région partagé pour extraire les headers gRPC
        .add_service(ProfileIdentityServiceServer::with_interceptor(
            identity_handler,
            region_interceptor
        ))
        .add_service(ProfileMediaServiceServer::with_interceptor(
            media_handler,
            region_interceptor
        ))
        .add_service(ProfileMetadataServiceServer::with_interceptor(
            metadata_handler,
            region_interceptor
        ))
        .serve(addr)
        .await?;

    Ok(())
}