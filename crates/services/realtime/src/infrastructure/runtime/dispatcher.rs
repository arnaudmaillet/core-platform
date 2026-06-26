//! The dispatcher fan-out consumer: the at-least-once runtime around
//! [`FanOutHandler`].
//!
//! Each upstream topic runs on the shared
//! [`run_consumer`](transport::kafka::consumer::run_consumer) runner (manual commit,
//! bounded retry + jitter, DLQ on poison/exhaustion). A decoded record is mapped to
//! a [`DeliverableEvent`](crate::application::DeliverableEvent) and fanned out. An
//! offline recipient is folded into `Ok` by the handler, so the offset commits;
//! only a fabric fault (`RTM-4001` / `RTM-4002`) retries.

use std::sync::Arc;

use transport::kafka::consumer::{ProcessOutcome, RetryPolicy, run_consumer, KafkaConsumerHandle};
use transport::kafka::envelope::ConsumablePayload;
use transport::kafka::producer::KafkaProducerHandle;

use crate::application::{DeliverableEvent, FanOutHandler};
use crate::error::RealtimeError;

/// Runs a fan-out consumer: decode each record with `map` and fan it out. The
/// runner owns deserialization, so poison bytes dead-letter before reaching `map`.
pub async fn run_fanout_consumer<T, M>(
    label: &'static str,
    consumer: KafkaConsumerHandle,
    producer: KafkaProducerHandle,
    handler: Arc<FanOutHandler>,
    map: M,
) where
    T: ConsumablePayload + Clone,
    M: Fn(T) -> Result<DeliverableEvent, RealtimeError> + Copy + Send + Sync + 'static,
{
    tracing::info!(label, "realtime dispatcher consumer started");
    let policy = RetryPolicy::default();
    let result = run_consumer::<T, _>(&consumer, &producer, &policy, move |event| {
        let handler = Arc::clone(&handler);
        let event = event.clone();
        Box::pin(async move {
            let outcome = async {
                let deliverable = map(event)?;
                handler.fan_out(&deliverable).await?;
                Ok::<(), RealtimeError>(())
            }
            .await;
            ProcessOutcome::from_result(outcome)
        })
    })
    .await;
    if let Err(error) = result {
        tracing::error!(label, %error, "realtime dispatcher consumer stopped");
    }
}
