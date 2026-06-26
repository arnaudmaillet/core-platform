//! Ingestion consumers — the async command path. Each runs on the shared
//! at-least-once [`run_consumer`](transport::kafka::consumer::run_consumer) runner
//! (manual commit, bounded retry + jitter, DLQ on poison/exhaustion) and is
//! self-spawned + supervised in [`crate::service`].
//!
//! Two consumers ship here: `post.v1.events` (content, hydrated via gRPC before
//! projection) and `moderation.v1.events` (visibility transitions, no hydration).
//! `profile.v1.events` is deferred — profile publishes no Kafka stream yet.

pub mod moderation_consumer;
pub mod post_consumer;

pub use moderation_consumer::run_moderation_consumer;
pub use post_consumer::run_post_consumer;

use crate::error::SearchError;

/// Lets the at-least-once runner classify a failure: delegate to the error's own
/// [`AppError::is_retryable`](error::AppError::is_retryable) verdict — transient
/// engine / source-service faults retry; decode / projection faults are poison and
/// dead-letter immediately.
impl transport::kafka::consumer::ClassifyError for SearchError {
    fn is_retryable(&self) -> bool {
        <Self as error::AppError>::is_retryable(self)
    }
}
