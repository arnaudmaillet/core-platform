use std::sync::Arc;

use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, RetryPolicy};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::producer::KafkaProducerHandle;
use uuid::Uuid;

use cqrs::{CommandBus, CqrsError, Envelope};

use crate::application::command::ingest_post_published::IngestPostPublishedCommand;
use crate::infrastructure::worker::{build_dlq_producer, dispatch_outcome};

const TOPIC: &str = "post.v1.events";

/// Event schema for the unified `post.v1.events` stream — the internally-tagged
/// `DomainEvent` from services/post: `{"type": "PostPublished"|"PostUpdated"|
/// "PostDeleted", ...}`. This worker fans out **PostPublished** and commits the
/// other variants without work (deletion is handled by `post_deleted_worker`).
///
/// `author_tier` is read with a default of `0` (Standard) for forward
/// compatibility: services/post does **not** currently denormalize a tier onto any
/// post topic, so every author is treated as Standard today and the VIP read-path
/// is dormant. If/when post emits `author_tier` (the documented
/// profile→geo-discovery→post chain), this worker honours it with no change.
#[derive(Debug, Deserialize)]
struct PostV1Event {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(default)]
    pub post_id: String,
    /// Also accepted as `author_id` in some event versions.
    #[serde(alias = "author_id", default)]
    pub profile_id: String,
    /// 0=Standard, 1=Premium, 2=Vip. Absent today → Standard.
    #[serde(default)]
    pub author_tier: u8,
    #[serde(default)]
    pub published_at_ms: i64,
    #[serde(default)]
    pub audio_id: Option<String>,
}

impl PostV1Event {
    fn is_published(&self) -> bool {
        self.event_type == "PostPublished"
    }
}

/// Long-lived Kafka consumer for the unified `post.v1.events` stream. Each
/// `PostPublished` event routes to `IngestPostPublishedCommand` via the command
/// bus, which fans out to Redis + ScyllaDB for Standard/Premium authors or
/// registers the post in the VIP ZSET for Vip authors. Other event types commit
/// without work.
///
/// Delivery semantics: at-least-once. All downstream writes are idempotent, so a
/// from-earliest replay on cutover safely re-materializes feeds without duplication.
pub struct PostPublishedWorker<CB> {
    kafka_config: KafkaClientConfig,
    command_bus:  Arc<CB>,
    group_id:     String,
}

impl<CB: CommandBus + 'static> PostPublishedWorker<CB> {
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
        run_consumer::<PostV1Event, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { dispatch_outcome(worker.process(event).await) })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process(&self, event: &PostV1Event) -> Result<(), CqrsError> {
        // The unified stream carries every post event; this worker only fans out
        // newly-published posts. Other types (Updated / Deleted) commit unprocessed.
        if !event.is_published() {
            return Ok(());
        }
        let cmd = IngestPostPublishedCommand {
            post_id:         event.post_id.clone(),
            author_id:       event.profile_id.clone(),
            author_tier:     event.author_tier,
            published_at_ms: event.published_at_ms,
            audio_id:        event.audio_id.clone(),
        };
        self.command_bus.dispatch(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_a_v1_post_published_event() {
        let json = r#"{"type":"PostPublished","post_id":"p1","profile_id":"a1","kind":"image","published_at_ms":1750000000000,"audio_id":"aud1","audio_kind":2}"#;
        let ev: PostV1Event = serde_json::from_str(json).unwrap();
        assert!(ev.is_published());
        assert_eq!(ev.post_id, "p1");
        assert_eq!(ev.profile_id, "a1");
        assert_eq!(ev.author_tier, 0); // post emits no tier today → Standard
        assert_eq!(ev.published_at_ms, 1_750_000_000_000);
        assert_eq!(ev.audio_id.as_deref(), Some("aud1"));
    }

    #[test]
    fn ignores_non_published_v1_events() {
        for json in [
            r#"{"type":"PostUpdated","post_id":"p1","profile_id":"a1"}"#,
            r#"{"type":"PostDeleted","post_id":"p1","profile_id":"a1"}"#,
        ] {
            let ev: PostV1Event = serde_json::from_str(json).unwrap();
            assert!(!ev.is_published());
        }
    }

    #[test]
    fn honors_author_tier_when_present() {
        // Forward-compat: if/when post denormalizes the tier, the worker reads it
        // and the VIP read-path activates with no further change.
        let json = r#"{"type":"PostPublished","post_id":"p1","profile_id":"a1","author_tier":2,"published_at_ms":1}"#;
        let ev: PostV1Event = serde_json::from_str(json).unwrap();
        assert_eq!(ev.author_tier, 2);
    }
}
