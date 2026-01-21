// backend/services/profile/query-server/src/main.rs

use std::sync::Arc;
use tonic::transport::Server;

// Application
use profile::application::update_username::UpdateUsernameUseCase;

// Infrastructure - API
use profile::infrastructure::api::grpc::handlers::IdentityHandler;
use profile::infrastructure::api::grpc::profile_v1::profile_identity_service_server::ProfileIdentityServiceServer;

// Infrastructure - Repositories (Spécifiques au Profile)
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
    let addr = "[::1]:50051".parse()?;

    // --- 1. INITIALISATION DES CLIENTS DB ---

    // Configuration et création de la pool Postgres (Shared Kernel)
    let mut db_config = DbConfig::from_env()?;
    db_config.max_connections = 5;
    let pool = create_postgres_pool(&db_config).await?;

    // Configuration et création de la session ScyllaDB (Shared Kernel)
    let scylla_config = ScyllaConfig::from_env()?;
    let scylla_session = create_scylla_session(&scylla_config).await?;

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
        profile_repo,   // Injection du Composite (Abstrait pour l'UseCase)
        outbox_repo,    // Injection de l'Outbox
        tx_manager,     // Injection du gestionnaire de transactions
    ));

    // --- 4. INITIALISATION DES HANDLERS (API) ---

    let identity_handler = IdentityHandler::new(update_username_usecase);

    // --- 5. DÉMARRAGE DU SERVEUR TONIC ---

    println!("🚀 Profile Query-Server listening on {}", addr);

    Server::builder()
        // Utilisation de l'intercepteur de région partagé pour extraire les headers gRPC
        .add_service(ProfileIdentityServiceServer::with_interceptor(
            identity_handler,
            region_interceptor
        ))
        .serve(addr)
        .await?;

    Ok(())
}