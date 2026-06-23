use std::sync::Arc;

use transport::kafka::config::client::KafkaClientConfig;
use transport::kafka::config::consumer::{AutoOffsetReset, ConsumerConfig};
use transport::kafka::consumer::builder::KafkaConsumerBuilder;
use transport::kafka::consumer::{run_consumer, ProcessOutcome, RetryPolicy};
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::port::RoutingRegistry;
use crate::domain::event::ConversationUnpublishedEvent;
use crate::domain::value_object::ConversationId;
use crate::infrastructure::streaming::ConversationBroadcastRegistry;
use crate::infrastructure::worker::build_dlq_producer;

const TOPIC: &str = "chat.conversation.unpublished";

/// Tears down the Audience Plane cluster-wide when a conversation is unpublished.
///
/// Every pod runs this consumer; on a `chat.conversation.unpublished` event it
/// (1) closes the local audience streams for that conversation — terminating
/// live guest connections everywhere, the blueprint's "cancel live guest
/// streams" — and (2) clears the audience-shard routing registry so publishers
/// immediately stop fanning the shadow. The Member Plane is untouched: members
/// keep interacting, the conversation is simply private again.
///
/// Follows the mandatory consumer-runtime standard: manual commit after a
/// terminal outcome, bounded retry with backoff + DLQ on a transient Redis
/// fault, immediate reject (dead-letter) of a malformed record.
pub struct VisibilityWorker {
    kafka_config:      KafkaClientConfig,
    audience_registry: Arc<ConversationBroadcastRegistry>,
    routing:           Arc<dyn RoutingRegistry>,
    group_id:          String,
}

impl VisibilityWorker {
    pub fn new(
        kafka_config:      KafkaClientConfig,
        audience_registry: Arc<ConversationBroadcastRegistry>,
        routing:           Arc<dyn RoutingRegistry>,
        group_id:          impl Into<String>,
    ) -> Self {
        Self { kafka_config, audience_registry, routing, group_id: group_id.into() }
    }

    pub async fn run(self) {
        let producer = match build_dlq_producer(&self.kafka_config) {
            Ok(producer) => producer,
            Err(e) => {
                tracing::error!(error = %e, "failed to build DLQ producer — visibility consumer not started");
                return;
            }
        };

        let worker = Arc::new(self);
        loop {
            match worker.clone().run_once(&producer).await {
                Ok(()) => tracing::warn!("visibility consumer exited cleanly — restarting"),
                Err(e) => {
                    tracing::error!(error = %e, "visibility consumer error — restarting after 5 s");
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

        tracing::info!(topic = TOPIC, group = %self.group_id, "visibility consumer started");

        let policy = RetryPolicy::default();
        run_consumer::<ConversationUnpublishedEvent, _>(&handle, producer, &policy, move |event| {
            let worker = Arc::clone(&self);
            Box::pin(async move { worker.process_one(event).await })
        })
        .await
        .map_err(|e| e.to_string())
    }

    async fn process_one(&self, event: &ConversationUnpublishedEvent) -> ProcessOutcome {
        let conversation_id = match ConversationId::try_from(event.conversation_id.as_str()) {
            Ok(c) => c,
            Err(_) => {
                return ProcessOutcome::Reject(format!(
                    "invalid conversation_id: '{}'",
                    event.conversation_id
                ));
            }
        };

        // Terminate local audience streams immediately (in-memory, infallible).
        self.audience_registry.close(&conversation_id);

        // Clear the routing registry (Redis); a transient failure is retried.
        ProcessOutcome::from_result(self.routing.clear(&conversation_id).await)
    }
}
