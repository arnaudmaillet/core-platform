use std::sync::Arc;

use futures_util::StreamExt;
use serde::Deserialize;
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};

use crate::application::port::{ReactionLedger, ScoreStore};
use crate::domain::value_object::PostId;

const TOPIC_CREATED: &str = "comment.created";
const TOPIC_DELETED: &str = "comment.deleted";

/// Minimal projection of comment service events consumed by the engagement service.
/// Unknown fields are silently ignored — the comment service schema is an external contract.
#[derive(Debug, Deserialize)]
pub struct CommentEngagementPayload {
    pub post_id: String,
}

/// Kafka consumer for comment lifecycle events from `services/comment`.
///
/// Drives `engagement:comments:{post_id}` Redis counter and the ScyllaDB
/// `post_interaction_counters.comment_count` column.
///
/// Unlike views/shares (which are batched), each comment event is individually
/// applied to ScyllaDB because comment frequency is orders of magnitude lower.
pub struct CommentEventConsumer<S, L> {
    kafka_config: KafkaClientConfig,
    score_store:  Arc<S>,
    ledger:       Arc<L>,
    group_id:     String,
}

impl<S: ScoreStore, L: ReactionLedger> CommentEventConsumer<S, L> {
    pub fn new(
        kafka_config: KafkaClientConfig,
        score_store:  Arc<S>,
        ledger:       Arc<L>,
        group_id:     impl Into<String>,
    ) -> Self {
        Self {
            kafka_config,
            score_store,
            ledger,
            group_id: group_id.into(),
        }
    }

    pub async fn run(self) {
        loop {
            match self.run_once().await {
                Ok(()) => {
                    tracing::warn!("comment event consumer exited cleanly — restarting");
                }
                Err(e) => {
                    tracing::error!(error = %e, "comment event consumer error — restarting after 5 s");
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
            .subscribe(TOPIC_CREATED)
            .subscribe(TOPIC_DELETED)
            .build()
            .map_err(|e| e.to_string())?;

        tracing::info!(
            topics = %format!("{}, {}", TOPIC_CREATED, TOPIC_DELETED),
            group  = %self.group_id,
            "comment event consumer started"
        );

        let mut stream = handle.stream::<CommentEngagementPayload>();

        while let Some(result) = stream.next().await {
            let envelope = match result {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!(error = %err, "comment event deserialization error — skipping");
                    continue;
                }
            };

            let delta: i64 = match envelope.topic.as_str() {
                TOPIC_CREATED =>  1,
                TOPIC_DELETED => -1,
                other => {
                    tracing::warn!(topic = other, "unexpected topic — skipping");
                    continue;
                }
            };

            if let Err(err) = self.apply(delta, &envelope.payload).await {
                tracing::error!(
                    error   = %err,
                    post_id = %envelope.payload.post_id,
                    delta,
                    "comment counter apply failed"
                );
            }
        }

        Ok(())
    }

    async fn apply(
        &self,
        delta:   i64,
        payload: &CommentEngagementPayload,
    ) -> Result<(), crate::error::EngagementError> {
        let post_id = PostId::try_from(payload.post_id.as_str())?;

        if delta > 0 {
            self.score_store.incr_comment(&post_id).await?;
        } else {
            self.score_store.decr_comment(&post_id).await?;
        }

        self.ledger
            .apply_interaction_delta(&post_id, 0, 0, delta)
            .await?;

        tracing::debug!(post_id = %post_id, delta, "comment count applied");

        Ok(())
    }
}
