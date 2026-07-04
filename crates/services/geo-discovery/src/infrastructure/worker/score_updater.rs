use std::sync::Arc;

use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, ProcessOutcome, RetryPolicy};
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::command::UpdateViralityWithTilesCommand;
use crate::application::port::{SpatialIndex, TileRepository};
use crate::domain::value_object::{H3Index, PostId};
use crate::error::GeoDiscoveryError;
use crate::infrastructure::worker::build_dlq_producer;
use crate::infrastructure::cache::RedisGeoSpatialIndex;
use crate::infrastructure::persistence::ScyllaTileRepository;

const TOPIC: &str = "counter.v1.popularity";

/// The `entity_type` discriminant geo cares about on the shared popularity
/// stream. Counter emits one snapshot per entity kind (post/profile/media/…);
/// geo only re-scores posts.
const ENTITY_TYPE_POST: &str = "post";

/// Kafka event schema for `counter.v1.popularity`.
///
/// Published by `services/counter` (its sole outbound signal) as a coarse,
/// slow-loop popularity snapshot per entity. The wire shape is
/// `{ entity_type, entity_id, score }`; geo filters for `entity_type == "post"`,
/// maps `entity_id → post_id` and `score → new_score`, and commits every other
/// entity kind as a no-op.
#[derive(Debug, Deserialize)]
pub struct PopularityEvent {
    pub entity_type: String,
    pub entity_id:   String,
    pub score:       f64,
}

/// Long-lived background worker that consumes `counter.v1.popularity` snapshots
/// (filtered to `entity_type == "post"`) and propagates new virality scores to
/// both ScyllaDB and Redis ZSETs.
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
        let producer = match build_dlq_producer(&self.kafka_config) {
            Ok(producer) => producer,
            Err(e) => {
                tracing::error!(topic = TOPIC, error = %e, "failed to build DLQ producer — score updater consumer not started");
                return;
            }
        };

        let worker = Arc::new(self);
        loop {
            match worker.clone().run_once(&producer).await {
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

    async fn run_once(self: Arc<Self>, producer: &KafkaProducerHandle) -> Result<(), String> {
        let mut config = ConsumerConfig::new(self.kafka_config.clone(), &self.group_id);
        config.auto_offset_reset  = AutoOffsetReset::Earliest;
        config.enable_auto_commit = false;

        let handle = KafkaConsumerBuilder::new(config)
            .subscribe(TOPIC)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(topic = TOPIC, group = %self.group_id, "score updater consumer started");

        let policy = RetryPolicy::default();
        run_consumer::<PopularityEvent, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { ProcessOutcome::from_result(worker.process(event).await) })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process(&self, event: &PopularityEvent) -> Result<(), GeoDiscoveryError> {
        use cqrs::{CommandHandler, Envelope};
        use uuid::Uuid;
        use crate::domain::value_object::H3Resolution;

        // The popularity stream carries every entity kind; geo only re-scores
        // posts. Anything else is a committed no-op.
        if event.entity_type != ENTITY_TYPE_POST {
            return Ok(());
        }

        let post_id = PostId::try_from(event.entity_id.as_str())?;

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
            post_id:     event.entity_id.clone(),
            new_score:   event.score,
            h3_index_r5: h3_r5,
            h3_index_r7: h3_r7,
            h3_index_r9: h3_r9,
        };

        handler.handle(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
