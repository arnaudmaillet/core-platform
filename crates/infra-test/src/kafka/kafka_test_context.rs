// crates/shared-kernel/src/test_utils/kafka_context.rs

use infra_kafka::rdkafka::config::ClientConfig;
use infra_kafka::rdkafka::message::{Header, OwnedHeaders};
use infra_kafka::rdkafka::producer::{FutureProducer, FutureRecord};
use serde_json::Value;
use std::time::Duration;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::kafka::{KAFKA_PORT, Kafka as KafkaImage};

use crate::KafkaTestContextBuilder;

pub struct KafkaTestContext {
    _container: ContainerAsync<KafkaImage>,
    bootstrap_servers: String,
    producer: FutureProducer,
}

impl KafkaTestContext {
    /// Donne accès au builder pour configurer le conteneur Kafka de test
    pub fn builder() -> KafkaTestContextBuilder {
        KafkaTestContextBuilder::new()
    }

    /// Instancie le conteneur et le producteur à partir de la configuration du builder
    pub async fn restore(builder: KafkaTestContextBuilder) -> Self {
        tracing::info!(
            "🐳 Démarrage du conteneur Docker Kafka ({}:{})...",
            builder.image_name,
            builder.image_tag
        );

        // 1. Initialisation et démarrage du conteneur avec Testcontainers
        let container = KafkaImage::default()
            .with_name(&builder.image_name)
            .with_tag(&builder.image_tag)
            .start()
            .await
            .expect("Impossible de démarrer le conteneur Kafka de test");

        // 2. Récupération du port hôte mappé dynamiquement par Docker
        let host_port = container
            .get_host_port_ipv4(KAFKA_PORT)
            .await
            .expect("Impossible de récupérer le port hôte de Kafka");

        let bootstrap_servers = format!("127.0.0.1:{}", host_port);
        tracing::info!("📡 Broker Kafka de test prêt sur : {}", bootstrap_servers);

        // 3. Construction du client de production rdkafka pour les injections de tests
        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &bootstrap_servers)
            .set("message.timeout.ms", "5000")
            .set("acks", "all");

        // Application des surcharges de configuration si le builder en contient
        if let Some(custom_cfg) = builder.config {
            for (key, val) in custom_cfg {
                client_config.set(key, val);
            }
        }

        let producer: FutureProducer = client_config
            .create()
            .expect("Impossible de créer le FutureProducer de test");

        Self {
            _container: container,
            bootstrap_servers,
            producer,
        }
    }

    pub fn bootstrap_servers(&self) -> &str {
        &self.bootstrap_servers
    }

    /// Publie un payload JSON brut sur un topic donné avec injection automatique du header d'événement
    pub async fn publish_raw(
        &self,
        topic: &str,
        key: &str,
        payload: Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let payload_str = serde_json::to_string(&payload)?;

        let event_type = payload
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown.event");

        let record = FutureRecord::to(topic)
            .payload(&payload_str)
            .key(key)
            .headers(OwnedHeaders::new().insert(Header {
                key: "event_type",
                value: Some(event_type),
            }));

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| e)?;

        tracing::debug!(topic = topic, key = key, "Message injecté avec succès.");
        Ok(())
    }
}
