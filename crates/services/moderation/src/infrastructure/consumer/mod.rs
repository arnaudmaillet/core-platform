//! Plane A ingestion consumers — the async, post-hoc path. Each runs on the
//! shared at-least-once [`run_consumer`](transport::kafka::consumer::run_consumer)
//! runner (manual commit, bounded retry + jitter, DLQ on poison/exhaustion) and
//! is self-spawned + supervised in [`crate::service`].
//!
//! Two consumers ship here — the moderation-owned topics whose schemas this
//! service defines: `moderation.reports` (user reports) and `moderation.signals`
//! (classifier verdicts). Fan-in from the content topics (`post`/`comment`/`chat`)
//! is a follow-up once a content-screen-on-create handler exists.

pub mod report_consumer;
pub mod signal_consumer;

pub use report_consumer::run_report_consumer;
pub use signal_consumer::run_signal_consumer;

use crate::error::ModerationError;

/// Lets the at-least-once runner classify a moderation failure: delegate to the
/// error's own [`AppError::is_retryable`](error::AppError::is_retryable) verdict
/// (transient storage/dependency faults retry; domain/validation faults are
/// poison and dead-letter immediately).
impl transport::kafka::consumer::ClassifyError for ModerationError {
    fn is_retryable(&self) -> bool {
        <Self as error::AppError>::is_retryable(self)
    }
}
