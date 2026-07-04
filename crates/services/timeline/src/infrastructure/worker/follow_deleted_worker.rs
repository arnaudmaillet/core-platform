use std::sync::Arc;

use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, RetryPolicy};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::producer::KafkaProducerHandle;
use uuid::Uuid;

use cqrs::{CommandBus, CqrsError, Envelope};

use crate::application::command::prune_follow::PruneFollowCommand;
use crate::infrastructure::worker::{build_dlq_producer, dispatch_outcome};

const TOPIC: &str = "social-graph.unfollowed";

/// Kafka event schema for `social-graph.unfollowed`.
#[derive(Debug, Deserialize)]
struct ProfileUnfollowedEvent {
    pub actor_id:  String,
    pub target_id: String,
}

/// Long-lived Kafka consumer for `social-graph.unfollowed`.
///
/// On each event:
///   - Removes `target_id`'s posts from `actor_id`'s Redis feed (Standard/Premium).
///   - Removes `target_id` from `timeline:following:{actor_id}` so subsequent
///     feed reads no longer merge their VIP ZSET.
///
/// VIP unfollows are handled purely by the following-set removal.
pub struct FollowDeletedWorker<CB> {
    kafka_config: KafkaClientConfig,
    command_bus:  Arc<CB>,
    group_id:     String,
}

impl<CB: CommandBus + 'static> FollowDeletedWorker<CB> {
    pub fn new(
        kafka_config: KafkaClientConfig,
        command_bus:  Arc<CB>,
        group_id:     impl Into<String>,
    ) -> Self {
        Self {
            kafka_config,
            command_bus,
            group_id: group_id.into(),
        }
    }

    pub async fn run(self) {
        let producer = match build_dlq_producer(&self.kafka_config) {
            Ok(producer) => producer,
            Err(e) => {
                tracing::error!(topic = TOPIC, error = %e, "failed to build DLQ producer — consumer not started");
                return;
            }
        };

        let worker = Arc::new(self);
        loop {
            match worker.clone().run_once(&producer).await {
                Ok(()) => {
                    tracing::warn!(topic = TOPIC, "consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(topic = TOPIC, error = %e, "consumer error — restarting after 5 s");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn run_once(self: Arc<Self>, producer: &KafkaProducerHandle) -> Result<(), String> {
        let mut config = ConsumerConfig::new(self.kafka_config.clone(), &self.group_id);
        config.auto_offset_reset  = AutoOffsetReset::Earliest;
        config.enable_auto_commit = false;

        let handle = KafkaConsumerBuilder::new(config)
            .subscribe(TOPIC)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(topic = TOPIC, group = %self.group_id, "consumer started");

        let policy = RetryPolicy::default();
        run_consumer::<ProfileUnfollowedEvent, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { dispatch_outcome(worker.process(event).await) })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process(&self, event: &ProfileUnfollowedEvent) -> Result<(), CqrsError> {
        let cmd = PruneFollowCommand {
            follower_id: event.actor_id.clone(),
            followee_id: event.target_id.clone(),
        };
        self.command_bus.dispatch(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
