// backend/services/profile/api/event-worker/src/main.rs

use dotenvy::dotenv;
use infra_fred::RedisContext;
use infra_kafka::KafkaEventConsumer;
use infra_sqlx::PostgresContext;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use shared_kernel::{
    core::Error,
    messaging::{EventConsumer, EventEnvelope},
};

use profile::{ProfileServiceBuilder, kafka::AccountConsumer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let kafka_group_id =
        std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "profile-worker-group".to_string());

    let max_concurrency = std::env::var("WORKER_MAX_CONCURRENCY")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()?;

    let pg_ctx = PostgresContext::builder()?
        .with_url(database_url)
        .with_max_connections(20)
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;
    let builder = ProfileServiceBuilder::new(pg_ctx.pool(), redis_ctx.repository());
    let app_ctx = builder.build_context();
    let bus = builder.build_command_bus();

    let account_consumer = Arc::new(AccountConsumer::new(bus.clone(), (*app_ctx).clone()));
    let kafka_transport_consumer =
        KafkaEventConsumer::new(&kafka_brokers, &kafka_group_id, max_concurrency);

    tracing::info!("👷 Profile Event Worker initialisé avec succès.");
    tracing::info!("📥 Connexion au broker Kafka : {}", kafka_brokers);

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
