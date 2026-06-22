use std::sync::Arc;

use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, RetryPolicy};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::producer::KafkaProducerHandle;
use uuid::Uuid;

use cqrs::{CommandBus, CqrsError, Envelope};

use crate::application::command::remove_post::RemovePostCommand;
use crate::infrastructure::worker::{build_dlq_producer, dispatch_outcome};

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

impl<CB: CommandBus + 'static> PostDeletedWorker<CB> {
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
        run_consumer::<PostDeletedEvent, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { dispatch_outcome(worker.process(event).await) })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process(&self, event: &PostDeletedEvent) -> Result<(), CqrsError> {
        let cmd = RemovePostCommand {
            post_id:         event.post_id.clone(),
            author_id:       event.profile_id.clone(),
            author_tier:     event.author_tier,
            published_at_ms: event.published_at_ms,
        };
        self.command_bus.dispatch(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
