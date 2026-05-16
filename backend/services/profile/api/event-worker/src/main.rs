// backend/services/profile/api/event-worker/src/main.rs

use dotenvy::dotenv;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

// Imports du Shared Kernel (Socle technique & Transport de la plateforme)
use shared_kernel::{
    kafka::KafkaEventConsumer,
    messaging::{EventConsumer, EventEnvelope},
    postgres::PostgresContext,
    redis::RedisContext,
};

// Imports de la crate Profile
use profile::{ProfileServiceBuilder, kafka::AccountConsumer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialisation des variables d'environnement et du logging
    dotenv().ok();
    tracing_subscriber::fmt::init();

    // 2. Récupération de la configuration de l'infrastructure
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set");
    let kafka_brokers =
        std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let kafka_group_id =
        std::env::var("KAFKA_GROUP_ID").unwrap_or_else(|_| "profile-worker-group".to_string());

    // Limite de concurrence globale par instance de worker (ex: max 100 messages traités en parallèle)
    let max_concurrency = std::env::var("WORKER_MAX_CONCURRENCY")
        .unwrap_or_else(|_| "100".to_string())
        .parse::<usize>()?;

    // 3. Initialisation des Contextes de Stockage
    let pg_ctx = PostgresContext::builder()?
        .with_url(database_url)
        .with_max_connections(20) // Ajuste selon la concurrence de ton worker
        .build()
        .await?;

    let redis_ctx = RedisContext::builder()?.with_url(redis_url).build().await?;

    // 4. Assemblage de l'Application via le ServiceBuilder existant
    let builder = ProfileServiceBuilder::new(pg_ctx.pool(), redis_ctx.repository());
    let app_ctx = builder.build_context(); // Notre ProfileAppContext unifié
    let bus = builder.build_command_bus(); // Notre CommandBus avec CreateProfileHandler

    // 5. Initialisation du Consumer de la couche d'infrastructure de Profile
    let account_consumer = Arc::new(AccountConsumer::new(bus.clone(), (*app_ctx).clone()));

    // 6. Instanciation du Moteur de Transport Kafka du Shared Kernel
    let kafka_transport_consumer =
        KafkaEventConsumer::new(&kafka_brokers, &kafka_group_id, max_concurrency);

    tracing::info!("👷 Profile Event Worker initialisé avec succès.");
    tracing::info!("📥 Connexion au broker Kafka : {}", kafka_brokers);

    // 7. Câblage de la fermeture propre (Graceful Shutdown)
    // Si tu envoies un SIGINT (Ctrl+C) ou SIGTERM, on arrête proprement d'écouter Kafka
    let shutdown_token = CancellationToken::new();
    let s_token = shutdown_token.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Impossible d'écouter le signal Ctrl+C");
        tracing::info!("🛑 Signal d'arrêt reçu, fermeture du consommateur Kafka...");
        s_token.cancel();
    });

    // 8. Définition de la closure d'adaptation (Le pont entre le Shared Kernel et ton Consumer local)
    // Le Shared Kernel fournit une EventEnvelope générique, on extrait le JSON brut (payload)
    // et on le passe à notre AccountConsumer dédié.
    let handler = {
        let consumer_adapter = Arc::clone(&account_consumer);
        Box::new(move |envelope: EventEnvelope| {
            let consumer = Arc::clone(&consumer_adapter);

            // CORRECTION : On type explicitement la boîte pour forcer la coercition vers le trait object attendu
            let fut: std::pin::Pin<
                Box<
                    dyn std::future::Future<Output = Result<(), shared_kernel::core::Error>> + Send,
                >,
            > = Box::pin(async move {
                // On ré-encode temporairement en bytes pour que ton AccountConsumer local
                // puisse désérialiser son contrat d'enum interne (AccountIncomingEvent) de manière isolée.
                let raw_payload = serde_json::to_vec(&envelope.payload)
                    .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

                consumer
                    .on_message_received(&raw_payload)
                    .await
                    .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

                Ok(())
            });

            fut
        })
    };

    // 9. Lancement de la boucle d'écoute infinie sur le topic cible
    let topic_target = "account.events"; // Modifie si ton topic a un autre nom
    tracing::info!(
        "📡 Écoute active sur le topic : '{}' (Groupe : '{}')",
        topic_target,
        kafka_group_id
    );

    // Exécution de la boucle. Si le shutdown_token est annulé par le Ctrl+C, consume() s'arrête proprement.
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
