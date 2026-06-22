use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use uuid::Uuid;

use cqrs::{CommandBus, Envelope};

use crate::application::command::ingest_post_published::IngestPostPublishedCommand;

const TOPIC: &str = "post.published";

/// Kafka event schema for `post.published`.
///
/// Published by services/post when a post transitions to the Published state.
/// `author_tier` is denormalized by services/post from services/profile
/// and propagated forward to avoid synchronous lookups on the timeline write path.
/// Absent (pre-tier-support) events default to `author_tier = 0` (Standard).
#[derive(Debug, Deserialize)]
struct PostPublishedEvent {
    pub post_id:    String,
    /// Also called `author_id` in some event versions. Both aliases are accepted.
    #[serde(alias = "author_id")]
    pub profile_id: String,
    /// 0=Standard, 1=Premium, 2=Vip. Absent in legacy events → Standard.
    #[serde(default)]
    pub author_tier: u8,
    pub published_at_ms: i64,
}

/// Long-lived Kafka consumer for `post.published`.
///
/// On each event:
///   - Routes to `IngestPostPublishedCommand` via the command bus.
///   - The command handler fan-outs to Redis + ScyllaDB for Standard/Premium
///     authors, or registers in the VIP ZSET for Vip authors.
///
/// Delivery semantics: at-least-once. All downstream writes are idempotent.
pub struct PostPublishedWorker<CB> {
    kafka_config: KafkaClientConfig,
    command_bus:  Arc<CB>,
    group_id:     String,
}

impl<CB: CommandBus> PostPublishedWorker<CB> {
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

        let mut stream = handle.stream::<PostPublishedEvent>();

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

            let cmd = IngestPostPublishedCommand {
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
                    topic     = TOPIC,
                    post_id   = %event.post_id,
                    error     = %e,
                    "IngestPostPublishedCommand failed — message will be redelivered"
                );
            }
        }

        Ok(())
    }
}
