use std::sync::Arc;

use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, ProcessOutcome, RetryPolicy};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::IndexPostCommand;
use crate::application::port::{CardStore, SpatialIndex, TileRepository};
use crate::infrastructure::cache::{RedisCardStore, RedisGeoSpatialIndex};
use crate::infrastructure::persistence::ScyllaTileRepository;
use crate::infrastructure::worker::build_dlq_producer;

const TOPIC: &str = "post.published";

/// Kafka event schema for `post.published`.
///
/// Published by `services/post` when a post transitions to Published status.
/// All fields required for spatial indexing and card projection are included.
#[derive(Debug, Deserialize)]
pub struct PostPublishedEvent {
    pub post_id:           String,
    pub author_id:         String,
    pub author_handle:     String,
    pub author_avatar_url: String,
    pub thumbnail_url:     String,
    pub lat:               f64,
    pub lng:               f64,
    /// Initial virality score. Typically 0.0 for brand-new posts.
    pub virality_score:    f64,
    /// Unix epoch milliseconds of the publication timestamp.
    pub published_at_ms:   i64,
    /// Optional retention override in seconds. Absent → 172 800 s (48 h).
    pub retention_secs:    Option<u64>,
    /// Author tier at publish time. 0=Standard, 1=Premium, 2=VIP.
    /// Denormalized by services/post from services/profile. Absent → 0 (Standard).
    #[serde(default)]
    pub author_tier:       u8,
}

/// Long-lived background worker that consumes `post.published` events and
/// indexes each post into the spatial index and card store.
///
/// Delivery semantics: at-least-once (auto-commit enabled). All writes are
/// idempotent (ZADD + cap Lua, ScyllaDB INSERT with no IF conditions), so
/// duplicate deliveries are safe.
pub struct PostIndexerWorker<SI, CS, TR> {
    kafka_config:        KafkaClientConfig,
    spatial_index:       Arc<SI>,
    card_store:          Arc<CS>,
    tile_repository:     Arc<TR>,
    group_id:            String,
    card_cache_threshold: f64,
}

impl PostIndexerWorker<RedisGeoSpatialIndex, RedisCardStore, ScyllaTileRepository> {
    pub fn new(
        kafka_config:        KafkaClientConfig,
        spatial_index:       Arc<RedisGeoSpatialIndex>,
        card_store:          Arc<RedisCardStore>,
        tile_repository:     Arc<ScyllaTileRepository>,
        group_id:            impl Into<String>,
        card_cache_threshold: f64,
    ) -> Self {
        Self {
            kafka_config,
            spatial_index,
            card_store,
            tile_repository,
            group_id: group_id.into(),
            card_cache_threshold,
        }
    }
}

impl<SI, CS, TR> PostIndexerWorker<SI, CS, TR>
where
    SI: SpatialIndex + 'static,
    CS: CardStore + 'static,
    TR: TileRepository + 'static,
{
    pub async fn run(self) {
        let producer = match build_dlq_producer(&self.kafka_config) {
            Ok(producer) => producer,
            Err(e) => {
                tracing::error!(topic = TOPIC, error = %e, "failed to build DLQ producer — post indexer consumer not started");
                return;
            }
        };

        let worker = Arc::new(self);
        loop {
            match worker.clone().run_once(&producer).await {
                Ok(()) => {
                    tracing::warn!(topic = TOPIC, "post indexer consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(topic = TOPIC, error = %e, "post indexer consumer error — restarting after 5 s");
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "post indexer consumer started");

        let policy = RetryPolicy::default();
        run_consumer::<PostPublishedEvent, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { ProcessOutcome::from_result(worker.process(event).await) })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process(&self, event: &PostPublishedEvent) -> Result<(), crate::error::GeoDiscoveryError> {
        use cqrs::{CommandHandler, Envelope};
        use uuid::Uuid;

        let handler = crate::application::command::IndexPostHandler {
            spatial_index:        Arc::clone(&self.spatial_index),
            card_store:           Arc::clone(&self.card_store),
            tile_repository:      Arc::clone(&self.tile_repository),
            card_cache_threshold: self.card_cache_threshold,
        };

        let cmd = IndexPostCommand {
            post_id:           event.post_id.clone(),
            author_id:         event.author_id.clone(),
            author_handle:     event.author_handle.clone(),
            author_avatar_url: event.author_avatar_url.clone(),
            thumbnail_url:     event.thumbnail_url.clone(),
            lat:               event.lat,
            lng:               event.lng,
            virality_score:    event.virality_score,
            published_at_ms:   event.published_at_ms,
            retention_secs:    event.retention_secs,
            author_tier:       event.author_tier,
        };

        handler.handle(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
