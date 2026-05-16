use async_trait::async_trait;
use chrono::Utc;
use profile::ProfileServiceBuilder;
use profile::kafka::AccountConsumer;
use profile::test_utils::ProfileTestContext;
use serde_json::json;
use shared_kernel::cache::CacheRepository;
use shared_kernel::core::Result;
use shared_kernel::kafka::{KafkaEventConsumer, KafkaEventProducer};
use shared_kernel::messaging::{EventConsumer, EventEnvelope, EventProducer};
use shared_kernel::test_utils::E2EServerStarter;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use uuid::Uuid;

struct ProfileWorkerStarter {
    bootstrap_servers: String,
}

#[async_trait]
impl E2EServerStarter for ProfileWorkerStarter {
    async fn start_server(
        &self,
        pg_pool: sqlx::PgPool,
        redis_repo: Arc<dyn CacheRepository>,
        _addr: SocketAddr,
        shutdown_rx: oneshot::Receiver<()>,
    ) {
        tracing::info!("Initializing Profile worker domain dependencies...");
        let builder = ProfileServiceBuilder::new(pg_pool, redis_repo);
        let app_ctx = builder.build_context();
        let bus = builder.build_command_bus();

        let account_consumer = Arc::new(AccountConsumer::new(bus.clone(), (*app_ctx).clone()));

        tracing::info!(bootstrap_servers = %self.bootstrap_servers, "Connecting technical KafkaEventConsumer...");
        let kafka_transport =
            KafkaEventConsumer::new(&self.bootstrap_servers, "profile-worker-test-group", 10);

        // Gestion de la fermeture propre du thread de test
        let shutdown_token = tokio_util::sync::CancellationToken::new();
        let token_clone = shutdown_token.clone();
        tokio::spawn(async move {
            shutdown_rx.await.ok();
            token_clone.cancel();
        });

        // Pont technique : Adaptation de la EventEnvelope générique vers le handler du domaine
        let handler = Box::new(move |envelope: EventEnvelope| {
            let consumer = Arc::clone(&account_consumer);

            let fut: std::pin::Pin<
                Box<dyn std::future::Future<Output = shared_kernel::core::Result<()>> + Send>,
            > = Box::pin(async move {
                let raw_payload = serde_json::to_vec(&envelope.payload)
                    .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

                consumer
                    .on_message_received(&raw_payload)
                    .await
                    .map_err(|e| shared_kernel::core::Error::internal(e.to_string()))?;

                tracing::info!(event_id = %envelope.id, "Event successfully dispatched to the domain consumer.");
                Ok(())
            });
            fut
        });

        tracing::info!("Kafka consumer loop active on topic 'account.events'.");
        if let Err(e) = kafka_transport.consume("account.events", handler).await {
            tracing::error!(error = ?e, "Technical Kafka consumer loop crashed");
        }
    }
}

#[tokio::test]
async fn test_worker_e2e_profile_creation_on_account_event() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // 1. SETUP INFRA
    let ctx = ProfileTestContext::builder().with_kafka().build().await;
    let bootstrap_servers = ctx.kernel().kafka().bootstrap_servers().to_string();

    // BOOT WORKER
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let pg_pool = ctx.pg_pool();
    let redis_repo = ctx.kernel().redis().repository();
    let starter = ProfileWorkerStarter {
        bootstrap_servers: bootstrap_servers.clone(),
    };

    tokio::spawn(async move {
        starter
            .start_server(pg_pool, redis_repo, "[::1]:0".parse().unwrap(), shutdown_rx)
            .await;
    });

    // 2. INITIALISATION DU PRODUCER OFFICIEL
    // On instancie le producteur du shared-kernel branché sur le même broker de test
    let kafka_producer = KafkaEventProducer::new(&bootstrap_servers, "account.events".to_string())
        .await
        .expect("Failed to create KafkaEventProducer");

    // 3. ACT : On crée une vraie EventEnvelope typée, sans JSON magique à la main
    let account_id = Uuid::new_v4();
    let region = "EU";

    // Le payload métier interne de ton domaine Account
    let account_created_payload = json!({
        "type": "account.created",
        "data": {
            "account_id": account_id.to_string(),
            "region": region,
            "username": "charlie_brown"
        }
    });

    // On bâtit l'enveloppe officielle
    let envelope = EventEnvelope {
        id: Uuid::new_v4(),
        event_type: "account.created".to_string(),
        occurred_at: Utc::now(),
        region_code: region.to_string(),
        aggregate_type: "Account".to_string(),
        aggregate_id: account_id.to_string(),
        payload: account_created_payload,
        metadata: None,
    };

    // On publie via le trait EventProducer officiel !
    tracing::info!(account_id = %account_id, "Publishing event using KafkaEventProducer...");
    kafka_producer
        .publish(&envelope)
        .await
        .expect("Failed to publish event");

    // 4. ASYNC WAIT / ASSERT (Le polling reste identique)
    let mut profile_created = false;
    for _ in 0..100 {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT handle FROM user_profiles WHERE account_id = $1")
                .bind(account_id)
                .fetch_optional(&ctx.pg_pool())
                .await
                .unwrap();

        if let Some(r) = row {
            tracing::info!(handle = %r.0, "Profile found in database !");
            profile_created = true;
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    assert!(
        profile_created,
        "The worker failed to process the message published by KafkaEventProducer"
    );

    let _ = shutdown_tx.send(());
    ctx.shutdown().await;
    Ok(())
}
