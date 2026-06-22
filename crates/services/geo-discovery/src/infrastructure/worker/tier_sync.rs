use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};

use crate::application::command::SyncAuthorTierCommand;
use crate::application::port::{CardStore, TileRepository};
use crate::infrastructure::cache::RedisCardStore;
use crate::infrastructure::persistence::ScyllaTileRepository;

const TOPIC: &str = "profile.tier_changed";

/// Kafka event schema for `profile.tier_changed`.
///
/// Published by `services/profile` when an author's tier changes (Standard →
/// Premium, Premium → VIP, or any downgrade). One event is emitted **per
/// post_id** so this consumer remains stateless — no author→posts index needed.
///
/// The author_id field is informational (useful for tracing) but not required
/// for the update, which is keyed by post_id.
#[derive(Debug, Deserialize)]
pub struct TierChangedEvent {
    pub author_id: String,
    pub post_id:   String,
    /// New tier value. 0=Standard, 1=Premium, 2=VIP.
    pub new_tier:  u8,
}

/// Long-lived background worker that consumes `profile.tier_changed` events
/// and propagates new author tier values into `map_post_cards` (ScyllaDB)
/// while invalidating the corresponding Redis card cache entry.
///
/// Delivery semantics: at-least-once (auto-commit enabled). The ScyllaDB
/// UPDATE is idempotent (last-write-wins). The Redis DEL is also idempotent.
/// Duplicate deliveries are safe.
pub struct TierSyncWorker<CS, TR> {
    kafka_config:    KafkaClientConfig,
    card_store:      Arc<CS>,
    tile_repository: Arc<TR>,
    group_id:        String,
}

impl TierSyncWorker<RedisCardStore, ScyllaTileRepository> {
    pub fn new(
        kafka_config:    KafkaClientConfig,
        card_store:      Arc<RedisCardStore>,
        tile_repository: Arc<ScyllaTileRepository>,
        group_id:        impl Into<String>,
    ) -> Self {
        Self {
            kafka_config,
            card_store,
            tile_repository,
            group_id: group_id.into(),
        }
    }
}

impl<CS, TR> TierSyncWorker<CS, TR>
where
    CS: CardStore + 'static,
    TR: TileRepository + 'static,
{
    pub async fn run(self) {
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!(topic = TOPIC, "tier sync consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(topic = TOPIC, error = %e, "tier sync consumer error — restarting after 5 s");
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "tier sync consumer started");

        let mut stream = handle.stream::<TierChangedEvent>();

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
                    topic     = TOPIC,
                    post_id   = envelope.key,
                    error     = %err,
                    "tier sync failed — message will be redelivered on consumer restart"
                );
            }
        }

        Ok(())
    }

    async fn process(&self, event: &TierChangedEvent) -> Result<(), crate::error::GeoDiscoveryError> {
        use cqrs::{CommandHandler, Envelope};
        use uuid::Uuid;

        let handler = crate::application::command::SyncAuthorTierHandler {
            card_store:      Arc::clone(&self.card_store),
            tile_repository: Arc::clone(&self.tile_repository),
        };

        let cmd = SyncAuthorTierCommand {
            post_id:  event.post_id.clone(),
            new_tier: event.new_tier,
        };

        handler.handle(Envelope::new(Uuid::now_v7(), cmd)).await
    }
}
