use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};

use crate::application::command::prune_follow::PruneFollowCommand;

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

impl<CB: CommandBus> FollowDeletedWorker<CB> {
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
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!(topic = TOPIC, "consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(
                        topic = TOPIC,
                        error = %e,
                        "consumer error — restarting after 5 s"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn run_once(&self) -> Result<(), String> {
        let mut config = ConsumerConfig::new(self.kafka_config.clone(), &self.group_id);
        config.auto_offset_reset  = AutoOffsetReset::Earliest;
        config.enable_auto_commit = true;

        let handle = KafkaConsumerBuilder::new(config)
            .subscribe(TOPIC)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(topic = TOPIC, group = %self.group_id, "consumer started");

        let mut stream = handle.stream::<ProfileUnfollowedEvent>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e)    => e,
                Err(err) => {
                    tracing::warn!(
                        topic = TOPIC,
                        error = %err,
                        "deserialization error — skipping message"
                    );
                    continue;
                }
            };

            let event = &envelope.payload;

            let cmd = PruneFollowCommand {
                follower_id: event.actor_id.clone(),
                followee_id: event.target_id.clone(),
            };

            if let Err(e) = self
                .command_bus
                .dispatch(Envelope::new(Uuid::now_v7(), cmd))
                .await
            {
                tracing::error!(
                    topic       = TOPIC,
                    follower_id = %event.actor_id,
                    followee_id = %event.target_id,
                    error       = %e,
                    "PruneFollowCommand failed"
                );
            }
        }

        Ok(())
    }
}
