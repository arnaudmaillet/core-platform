use std::sync::Arc;

use futures_util::StreamExt;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};

use crate::application::port::ReactionLedger;
use crate::domain::event::reaction_event::ReactionKafkaEvent;
use crate::domain::value_object::{PostId, ProfileId};

const TOPIC: &str = "engagement.reactions";

/// Kafka consumer that durably applies reaction events to the ScyllaDB ledger.
///
/// This worker runs as a long-lived background task spawned at service startup.
/// It enforces at-least-once delivery semantics: Kafka offsets are committed
/// automatically after each batch interval (`enable_auto_commit = true`).
///
/// The ledger UPSERT is idempotent (last-write-wins), making redelivery safe.
/// Removal operations are also safe to retry — deleting a non-existent row is
/// a no-op in ScyllaDB.
pub struct ReactionWriteBehindWorker<L> {
    kafka_config: KafkaClientConfig,
    ledger:       Arc<L>,
    group_id:     String,
}

impl<L: ReactionLedger> ReactionWriteBehindWorker<L> {
    pub fn new(kafka_config: KafkaClientConfig, ledger: Arc<L>, group_id: impl Into<String>) -> Self {
        Self {
            kafka_config,
            ledger,
            group_id: group_id.into(),
        }
    }

    /// Runs indefinitely, consuming `engagement.reactions` events and writing
    /// to ScyllaDB. Call this inside `tokio::spawn`.
    pub async fn run(self) {
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!("reaction write-behind consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(error = %e, "reaction write-behind consumer error — restarting after 5 s");
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "reaction write-behind consumer started");

        let mut stream = handle.stream::<ReactionKafkaEvent>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(error = %err, "deserialization error — skipping message");
                    continue;
                }
            };

            if let Err(err) = self.process(&envelope.payload).await {
                tracing::error!(
                    error     = %err,
                    post_id   = envelope.key,
                    "ledger write failed — message will be redelivered on consumer restart"
                );
            }
        }

        Ok(())
    }

    async fn process(&self, event: &ReactionKafkaEvent) -> Result<(), crate::error::EngagementError> {
        match event {
            ReactionKafkaEvent::Upserted(e) => {
                let post_id    = PostId::try_from(e.post_id.as_str())?;
                let profile_id = ProfileId::try_from(e.profile_id.as_str())?;

                self.ledger
                    .upsert(&post_id, &profile_id, e.new_kind, e.new_weight, e.event_at_ms)
                    .await?;

                tracing::debug!(
                    post_id    = %post_id,
                    profile_id = %profile_id,
                    kind       = e.new_kind.as_redis_key(),
                    "ledger upsert applied"
                );
            }

            ReactionKafkaEvent::Removed(e) => {
                let post_id    = PostId::try_from(e.post_id.as_str())?;
                let profile_id = ProfileId::try_from(e.profile_id.as_str())?;

                self.ledger.remove(&post_id, &profile_id).await?;

                tracing::debug!(
                    post_id    = %post_id,
                    profile_id = %profile_id,
                    "ledger removal applied"
                );
            }
        }

        Ok(())
    }
}
