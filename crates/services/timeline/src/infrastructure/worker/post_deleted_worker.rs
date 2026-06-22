use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};

use crate::application::command::remove_post::RemovePostCommand;

const TOPIC: &str = "post.deleted";

/// Kafka event schema for `post.deleted`.
///
/// Published by services/post when a post is soft-deleted or hard-deleted.
/// `author_tier` and `published_at_ms` may be absent in legacy schemas;
/// defaults are applied for backwards compatibility.
#[derive(Debug, Deserialize)]
struct PostDeletedEvent {
    pub post_id:  String,
    #[serde(alias = "author_id")]
    pub profile_id: String,
    #[serde(default)]
    pub author_tier: u8,
    /// Needed for ScyllaDB DELETE (clustering key). Absent → 0 (best-effort delete).
    #[serde(default)]
    pub published_at_ms: i64,
    #[serde(alias = "deleted_at_ms")]
    pub _deleted_at_ms: Option<i64>,
}

pub struct PostDeletedWorker<CB> {
    kafka_config: KafkaClientConfig,
    command_bus:  Arc<CB>,
    group_id:     String,
}

impl<CB: CommandBus> PostDeletedWorker<CB> {
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

        let mut stream = handle.stream::<PostDeletedEvent>();

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

            let cmd = RemovePostCommand {
                post_id:         event.post_id.clone(),
                author_id:       event.profile_id.clone(),
                author_tier:     event.author_tier,
                published_at_ms: event.published_at_ms,
            };

            if let Err(e) = self
                .command_bus
                .dispatch(Envelope::new(Uuid::now_v7(), cmd))
                .await
            {
                tracing::error!(
                    topic   = TOPIC,
                    post_id = %event.post_id,
                    error   = %e,
                    "RemovePostCommand failed — offset NOT committed; will redeliver"
                );
                return Err(format!("dispatch failed: {e}"));
            }

            handle.commit(&msg).map_err(|e| e.to_string())?;
        }

        Ok(())
    }
}
