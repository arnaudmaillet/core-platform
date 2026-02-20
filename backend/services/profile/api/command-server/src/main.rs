// backend/services/profile/api/command-server/src/main.rs

use profile::application::use_cases::remove_avatar::RemoveAvatarUseCase;
use profile::application::use_cases::remove_banner::RemoveBannerUseCase;
use profile::application::use_cases::update_avatar::UpdateAvatarUseCase;
use profile::application::use_cases::update_banner::UpdateBannerUseCase;
use profile::application::use_cases::update_bio::UpdateBioUseCase;
use profile::application::use_cases::update_display_name::UpdateDisplayNameUseCase;
use profile::application::use_cases::update_location_label::UpdateLocationLabelUseCase;
use profile::application::use_cases::update_privacy::UpdatePrivacyUseCase;
use profile::application::use_cases::update_social_links::UpdateSocialLinksUseCase;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_reflection::server::Builder;
use profile::application::use_cases::update_handle::UpdateHandleUseCase;
use profile::infrastructure::api::grpc::SERVICE_DESCRIPTOR_SET;

// Infrastructure - API
use profile::infrastructure::api::grpc::handlers::{IdentityHandler, MediaHandler, MetadataHandler, };
use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_server::ProfileIdentityServiceServer;
use profile::infrastructure::api::grpc::profile_v1::profile_media_service_server::ProfileMediaServiceServer;
use profile::infrastructure::api::grpc::profile_v1::profile_metadata_service_server::ProfileMetadataServiceServer;
use profile::infrastructure::persistence_orchestrator::UnifiedProfileRepository;
use profile::infrastructure::postgres::repositories::PostgresIdentityRepository;

// Infrastructure - Repositories (Spécifiques au Profile)
use profile::infrastructure::postgres::utils::run_postgres_migrations;
use profile::infrastructure::scylla::repositories::ScyllaProfileRepository;
use profile::infrastructure::scylla::utils::run_scylla_migrations;

// Shared Kernel
use shared_kernel::infrastructure::grpc::region_interceptor;
use shared_kernel::infrastructure::postgres::factories::PostgresContext;
use shared_kernel::infrastructure::postgres::repositories::PostgresOutboxRepository;
use shared_kernel::infrastructure::postgres::transactions::PostgresTransactionManager;
use shared_kernel::infrastructure::redis::factories::RedisContext;
use shared_kernel::infrastructure::scylla::factories::ScyllaContext;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "50051".to_string());

    run_server(format!("0.0.0.0:{}", port).parse()?, ).await
}

pub async fn run_server(addr: std::net::SocketAddr, ) -> Result<(), Box<dyn std::error::Error>> {

    // --- INITIALISATION DU SERVICE DE SANTÉ ---

    let (health_reporter, health_service) = health_reporter();
    health_reporter.set_serving::<ProfileIdentityServiceServer<IdentityHandler>>().await;
    health_reporter.set_serving::<ProfileMediaServiceServer<MediaHandler>>().await;
    health_reporter.set_serving::<ProfileMetadataServiceServer<MetadataHandler>>().await;

    // --- INITIALISATION DU SERVICE DE RÉFLEXION ---
    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(SERVICE_DESCRIPTOR_SET)
        .build_v1()?;

    // --- 1. INITIALISATION DES CLIENTS DB ---

    // création de la pool Postgres (Shared Kernel)
    let pg_ctx = PostgresContext::builder()?.build().await?;
    run_postgres_migrations(&pg_ctx.pool()).await?;
    println!("✅ Postgres migrations completed.");

    // création de la session ScyllaDB (Shared Kernel)
    let scylla_ctx = ScyllaContext::builder()?.build().await?;
    run_scylla_migrations(&scylla_ctx.session()).await?;
    println!("✅ ScyllaDB migrations completed.");

    // création de Redis (Shared Kernel)
    let redis_ctx = RedisContext::builder()?.build().await?;
    println!("✅ Redis connection established.");

    // --- 2. INITIALISATION DES REPOSITORIES (Infrastructure) ---

    // Implémentations techniques (On clone les pools car elles sont conçues pour ça)
    let postgres_repository = Arc::new(PostgresIdentityRepository::new(pg_ctx.pool()));
    let scylla_repository = Arc::new(ScyllaProfileRepository::new(scylla_ctx.session()));
    let redis_repository = redis_ctx.repository();

    // L'orchestrateur (Façade Composite) qui masque la dualité DB au Domaine
    let profile_repo = Arc::new(UnifiedProfileRepository::new(
        postgres_repository,
        scylla_repository,
        redis_repository
    ));

    // Outils techniques partagés pour la cohérence des données (Transaction + Outbox)
    let tx_manager = Arc::new(PostgresTransactionManager::new(pg_ctx.pool()));
    let outbox_repo = Arc::new(PostgresOutboxRepository::new(pg_ctx.pool()));
    
    // --- 3. INITIALISATION DES USE CASES (Application) ---

    let update_username_use_case = Arc::new(UpdateHandleUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_display_name_use_case = Arc::new(UpdateDisplayNameUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_privacy_use_case = Arc::new(UpdatePrivacyUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_avatar_use_case = Arc::new(UpdateAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let remove_avatar_use_case = Arc::new(RemoveAvatarUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_banner_use_case = Arc::new(UpdateBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let remove_banner_use_case = Arc::new(RemoveBannerUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let bio_update_use_case = Arc::new(UpdateBioUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_location_label_use_case = Arc::new(UpdateLocationLabelUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));
    let update_social_links_use_case = Arc::new(UpdateSocialLinksUseCase::new(profile_repo.clone(), outbox_repo.clone(), tx_manager.clone(), ));

    // --- 4. INITIALISATION DES HANDLERS (API) ---

    let identity_handler = IdentityHandler::new(
        update_username_use_case,
        update_display_name_use_case,
        update_privacy_use_case,
    );
    let media_handler = MediaHandler::new(
        update_avatar_use_case,
        remove_avatar_use_case,
        update_banner_use_case,
        remove_banner_use_case,
    );
    let metadata_handler = MetadataHandler::new(
        bio_update_use_case,
        update_location_label_use_case,
        update_social_links_use_case,
    );

    // --- 5. DÉMARRAGE DU SERVEUR TONIC ---

    println!("🚀 Profile Query-Server listening on {}", addr);

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        // Utilisation de l'intercepteur de région partagé pour extraire les headers gRPC
        .add_service(ProfileIdentityServiceServer::with_interceptor(
            identity_handler,
            region_interceptor,
        ))
        .add_service(ProfileMediaServiceServer::with_interceptor(
            media_handler,
            region_interceptor,
        ))
        .add_service(ProfileMetadataServiceServer::with_interceptor(
            metadata_handler,
            region_interceptor,
        ))
        .serve(addr)
        .await?;

    Ok(())
}
