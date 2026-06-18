// backend/services/post/src/profile_projection_main.rs

use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_kafka::KafkaEventConsumer;
use infra_scylla::ScyllaContext;
use post::messaging::ProfileEventHandler;
use post::services::ProfileProjectionOrchestrator;
use post::{RedisProfileCache, ScyllaProfileProjection};
use shared_kernel::core::{Error, Result};
use shared_kernel::messaging::EventConsumer;
use shared_kernel::types::Region;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Alignement parfait du Tracing avec ton serveur de commande
    let _ =
        fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("info,scylla=debug,post=debug,infra_kafka=info")
            }))
            .try_init();

    dotenv().ok();

    info!("Starting Post Profile Projection Worker...");

    // 2. Récupération et validation du Sharding Régional (Identique à ton API)
    let region_str = std::env::var("CLUSTER_REGION")
        .or_else(|_| std::env::var("REGION"))
        .unwrap_or_else(|_| "EU".to_string());
    let local_region = Region::try_from(region_str.as_str())?;

    let scylla_nodes_str =
        std::env::var("POST_SCYLLA_NODES").unwrap_or_else(|_| "127.0.0.1:9042".to_string());
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());

    let scylla_nodes: Vec<String> = scylla_nodes_str
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    // Calcul déterministe du Keyspace souverain de la région (Strictement identique à ton API)
    let keyspace_name = format!("{}_post_storage", local_region.to_string().to_lowercase());

    // 3. Initialisation des Contextes d'Infrastructure Drivers officiels
    info!(
        "Initializing ScyllaDB Context for keyspace: {}",
        keyspace_name
    );
    let scylla_ctx = ScyllaContext::builder_from_env()?
        .with_nodes(scylla_nodes)
        .with_keyspace(&keyspace_name)
        .build()
        .await?;
    let session = scylla_ctx.session().clone();

    info!("Initializing Redis Context (Fred driver)...");
    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 4. Instanciation des dépôts (Repositories) basés sur tes vrais contextes
    let scylla_projection_store =
        Arc::new(ScyllaProfileProjection::new(session, keyspace_name).await?);
    let redis_profile_cache_store = Arc::new(RedisProfileCache::new(
        redis_ctx.cache_repository().pool().clone(),
    ));

    // 5. Assemblage de la couche applicative et du Handler de présentation
    let orchestrator = Arc::new(ProfileProjectionOrchestrator::new(
        scylla_projection_store,
        redis_profile_cache_store,
    ));

    let profile_handler = Arc::new(ProfileEventHandler::new(orchestrator, local_region));

    // 6. Configuration du consommateur Kafka d'infrastructure
    let consumer_group = "post-service-profile-projection";
    let topic_name = "global.profile.events.v1";
    let max_concurrency = 16;

    let kafka_consumer = Arc::new(KafkaEventConsumer::new(
        &kafka_brokers,
        consumer_group,
        max_concurrency,
    ));

    // 7. Gestion du Graceful Shutdown (SIGINT / SIGTERM)
    let shutdown_consumer = Arc::clone(&kafka_consumer);
    tokio::spawn(async move {
        let sig_int = signal::ctrl_c();
        #[cfg(unix)]
        let sig_term = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .unwrap()
                .recv()
                .await;
        };
        #[cfg(not(unix))]
        let sig_term = std::future::pending::<()>();

        tokio::select! {
            _ = sig_int => info!("SIGINT (Ctrl+C) caught, shutting down worker cleanly..."),
            _ = sig_term => info!("SIGTERM caught, shutting down worker cleanly..."),
        }

        shutdown_consumer.stop();
    });

    // 8. Lancement de la consommation
    info!(
        topic = %topic_name,
        group = %consumer_group,
        region = ?local_region,
        "Profile projection worker successfully bound to cluster context"
    );

    let handler_fn = move |envelope| {
        let handler = Arc::clone(&profile_handler);
        Box::pin(async move { handler.handle(envelope).await }) as _
    };

    if let Err(e) = kafka_consumer
        .consume(topic_name, Box::new(handler_fn))
        .await
    {
        error!(error = %e, "Fatal failure inside Kafka event loop");
        return Err(e);
    }

    info!("Post Profile Projection Worker stopped cleanly. Out.");
    Ok(())
}
