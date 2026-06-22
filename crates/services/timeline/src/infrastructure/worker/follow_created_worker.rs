use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};

use crate::application::command::backfill_follow::BackfillFollowCommand;

const TOPIC: &str = "social-graph.followed";

/// Kafka event schema for `social-graph.followed`.
///
/// Published by services/social-graph when a profile follows another profile.
/// `actor_id` is the follower; `target_id` is the new followee.
#[derive(Debug, Deserialize)]
struct ProfileFollowedEvent {
    pub actor_id:  String,
    pub target_id: String,
}

/// Long-lived Kafka consumer for `social-graph.followed`.
///
/// On each event:
///   - Injects `target_id`'s recent posts into `actor_id`'s Redis feed
///     (Standard/Premium only; VIP backfill is skipped as it's merged at read-time).
///   - Updates `timeline:following:{actor_id}` Redis SET for read-path routing.
///
/// Delivery semantics: at-least-once. All writes are idempotent.
pub struct FollowCreatedWorker<CB> {
    kafka_config: KafkaClientConfig,
    command_bus:  Arc<CB>,
    group_id:     String,
}

impl<CB: CommandBus> FollowCreatedWorker<CB> {
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
        // At-least-once: never auto-commit. The offset is advanced only after the
        // command has been successfully applied (see the commit calls below).
        config.enable_auto_commit = false;

        let handle = KafkaConsumerBuilder::new(config)
            .subscribe(TOPIC)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(topic = TOPIC, group = %self.group_id, "consumer started");

        let mut stream = handle.stream::<ProfileFollowedEvent>();

        while let Some(result) = stream.next().await {
            let msg = match result {
                Ok(m)    => m,
                Err(err) => {
                    tracing::warn!(topic = TOPIC, error = %err, "broker stream error — restarting consumer");
                    return Err(err.to_string());
                }
            };

            let event = match &msg.payload {
                Ok(e)    => e,
                Err(err) => {
                    tracing::warn!(
                        topic  = TOPIC,
                        offset = msg.offset,
                        error  = %err,
                        "deserialization error — committing past poison message"
                    );
                    handle.commit(&msg).map_err(|e| e.to_string())?;
                    continue;
                }
            };

            let cmd = BackfillFollowCommand {
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
                    "BackfillFollowCommand failed — offset NOT committed; will redeliver"
                );
                return Err(format!("dispatch failed: {e}"));
            }

            handle.commit(&msg).map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}
