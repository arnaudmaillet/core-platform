// backend/services/profile/api/event-worker/src/main.rs

use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_kafka::KafkaEventConsumer;
use infra_scylla::ScyllaContext;
use profile::stores::{ScyllaProfileRoutingStore, ScyllaProfileStore};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use shared_kernel::{
    command::CommandBus,
    core::Error,
    environment::ClusterContext,
    messaging::{EventConsumer, EventEnvelope},
    types::Region,
};

use profile::{ProfileServiceBuilder, kafka::AccountConsumer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // 1. Chargement et validation des variables d'environnement de Sharding & Kafka
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION")) // Uniformisation du fallback inter-services
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;
    let cluster_ctx = ClusterContext::new(local_region);

    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let kafka_group_id =
        std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "profile-worker-group".to_string());

    let max_concurrency = std::env::var("WORKER_MAX_CONCURRENCY")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()?;

    // 2. Initialisation des Contextes d'Infrastructure Drivers (Scylla & Redis)
    let scylla_ctx = ScyllaContext::builder_from_env()?.build().await?;
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 3. Instanciation des Adaptateurs (Repositories Concrets)
    let session = scylla_ctx.session().clone();
    let cache_repo = redis_ctx.cache_repository();
    let idempotency_repo = redis_ctx.idempotency_repository();

    let routing_store = Arc::new(
        ScyllaProfileRoutingStore::new(session.clone())
            .await
            .expect("Failed to prepare ScyllaDB global routing statements"),
    );

    // Calcul du nom du keyspace régional pour ce worker d'ingestion locale
    let keyspace_name = format!(
        "{}_profile_storage",
        local_region.to_string().to_lowercase()
    );

    let profile_store = Arc::new(
        ScyllaProfileStore::new(session, keyspace_name)
            .await
            .expect("Failed to prepare ScyllaDB regional profile statements"),
    );

    // 4. Assemblage via le Builder de Service et configuration du Kernel
    let builder = ProfileServiceBuilder::new(
        profile_store,
        routing_store,
        idempotency_repo.clone(),
        cluster_ctx,
    );

    let kernel = builder.build_kernel_ctx();

    // 5. Initialisation et configuration du CommandBus pour l'exécution des cas d'usage
    let mut command_bus = CommandBus::new(cache_repo, idempotency_repo);
    builder.register_handlers(&mut command_bus);
    let bus = Arc::new(command_bus);

    // 6. Initialisation des composants métiers du worker
    let account_consumer = Arc::new(AccountConsumer::new(bus.clone(), kernel));
    let kafka_transport_consumer =
        KafkaEventConsumer::new(&kafka_brokers, &kafka_group_id, max_concurrency);

    tracing::info!(
        "👷 Profile Event Worker Shard [{:?}] initialisé avec succès.",
        local_region
    );
    tracing::info!("📥 Connexion au broker Kafka : {}", kafka_brokers);

    // 7. Gestion propre du Cycle de Vie (Graceful Shutdown via CancellationToken)
    let shutdown_token = CancellationToken::new();
    let s_token = shutdown_token.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Impossible d'écouter le signal Ctrl+C");
        tracing::info!("🛑 Signal d'arrêt reçu, fermeture du consommateur Kafka...");
        s_token.cancel();
    });

    let handler = {
        let consumer_adapter = Arc::clone(&account_consumer);
        Box::new(move |envelope: EventEnvelope| {
            let consumer = Arc::clone(&consumer_adapter);
            let fut: std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), shared_kernel::core::Error>> + Send,
                >,
            > = Box::pin(async move {
                let raw_payload = serde_json::to_vec(&envelope.payload)
                    .map_err(|e| Error::internal(e.to_string()))?;

                consumer
                    .on_message_received(&raw_payload)
                    .await
                    .map_err(|e| Error::internal(e.to_string()))?;

                Ok(())
            });

            fut
        })
    };

    let topic_target = "account.events.v1";
    tracing::info!(
        "📡 Écoute active sur le topic : '{}' (Groupe : '{}')",
        topic_target,
        kafka_group_id
    );

    if let Err(err) = kafka_transport_consumer
        .consume(topic_target, handler)
        .await
    {
        tracing::error!(
            "💥 Erreur critique dans la boucle de consommation Kafka : {:?}",
            err
        );
    }

    tracing::info!("👋 Worker arrêté proprement. Ressources libérées.");
    Ok(())
}
