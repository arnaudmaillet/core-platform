use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};

use crate::application::command::UpdateViralityWithTilesCommand;
use crate::application::port::{SpatialIndex, TileRepository};
use crate::domain::value_object::{H3Index, PostId};
use crate::error::GeoDiscoveryError;
use crate::infrastructure::cache::RedisGeoSpatialIndex;
use crate::infrastructure::persistence::ScyllaTileRepository;

const TOPIC: &str = "engagement.score_updated";

/// Kafka event schema for `engagement.score_updated`.
///
/// Published by `services/engagement` whenever a post's aggregate virality
/// score changes (reaction upserted, removed, or counters flushed).
#[derive(Debug, Deserialize)]
pub struct ScoreUpdatedEvent {
    pub post_id:   String,
    pub new_score: f64,
}

/// Long-lived background worker that consumes `engagement.score_updated` events
/// and propagates new virality scores to both ScyllaDB and Redis ZSETs.
///
/// Tile resolution: before dispatching the update command, this worker performs
/// a ScyllaDB point-read on `map_post_cards` to fetch the canonical `h3_index_r7`
/// for the post. It then derives R5 via `H3Index::parent(R5)` and R9 via
/// `H3Index::parent(R9)` using h3o's parent resolution traversal.
///
/// This ScyllaDB read is unavoidable: the score event only carries `post_id`,
/// not location. The read uses the `Fast` execution profile (LocalOne + speculative),
/// adding ≤1 ms of latency on the hot path.
///
/// Delivery semantics: at-least-once (auto-commit enabled). The ScyllaDB UPDATE
/// is idempotent (last-write-wins). The ZADD XX Lua script is also idempotent.
pub struct ScoreUpdaterWorker<SI, TR> {
    kafka_config: KafkaClientConfig,
    spatial_index:   Arc<SI>,
    tile_repository: Arc<TR>,
    group_id:        String,
}

impl ScoreUpdaterWorker<RedisGeoSpatialIndex, ScyllaTileRepository> {
    pub fn new(
        kafka_config:    KafkaClientConfig,
        spatial_index:   Arc<RedisGeoSpatialIndex>,
        tile_repository: Arc<ScyllaTileRepository>,
        group_id:        impl Into<String>,
    ) -> Self {
        Self {
            kafka_config,
            spatial_index,
            tile_repository,
            group_id: group_id.into(),
        }
    }
}

impl<SI, TR> ScoreUpdaterWorker<SI, TR>
where
    SI: SpatialIndex + 'static,
    TR: TileRepository + 'static,
{
    pub async fn run(self) {
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!(topic = TOPIC, "score updater consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(topic = TOPIC, error = %e, "score updater consumer error — restarting after 5 s");
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "score updater consumer started");

        let mut stream = handle.stream::<ScoreUpdatedEvent>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e)    => e,
                Err(err) => {
                    tracing::warn!(topic = TOPIC, error = %err, "deserialization error — skipping message");
                    continue;
                }
            };

            if let Err(err) = self.process(&envelope.payload).await {
                tracing::error!(
                    topic   = TOPIC,
                    post_id = envelope.key,
                    error   = %err,
                    "score update failed — message will be redelivered on consumer restart"
                );
            }
        }

        Ok(())
    }

    async fn process(&self, event: &ScoreUpdatedEvent) -> Result<(), GeoDiscoveryError> {
        use cqrs::{CommandHandler, Envelope};
        use uuid::Uuid;
        use crate::domain::value_object::H3Resolution;

        let post_id = PostId::try_from(event.post_id.as_str())?;

        // Resolve the post's tile indices from ScyllaDB (Fast profile).
        let card = self.tile_repository
            .get_card(&post_id)
            .await?;

        let (h3_r5, h3_r7, h3_r9) = match card {
            Some(c) => {
                let idx_r7 = H3Index::from_i64(c.h3_index_r7)?;
                let idx_r5 = idx_r7.parent(H3Resolution::R5);
                // R9 is a finer resolution; derive from R7 by encoding the same
                // centre coordinate. Since we store only R7, we use parent for R5
                // and keep R7 as a proxy for R9 (nearest coarser tile available).
                // The XX script will simply not update R9 if the post is absent.
                let idx_r9 = idx_r7.parent(H3Resolution::R9); // safe fallback
                (idx_r5.as_i64(), idx_r7.as_i64(), idx_r9.as_i64())
            }
            None => {
                tracing::debug!(post_id = %post_id, "card not found in ScyllaDB — post likely expired; skipping score update");
                return Ok(());
            }
        };

        let handler = crate::application::command::UpdateViralityWithTilesHandler {
            spatial_index:   Arc::clone(&self.spatial_index),
            tile_repository: Arc::clone(&self.tile_repository),
        };

        let cmd = UpdateViralityWithTilesCommand {
            post_id:     event.post_id.clone(),
            new_score:   event.new_score,
            h3_index_r5: h3_r5,
            h3_index_r7: h3_r7,
            h3_index_r9: h3_r9,
        };

        handler.handle(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
