//! Generic at-least-once consumer runner with bounded retry and dead-lettering.
//!
//! This is the single place that owns the per-message state machine shared by every
//! Kafka worker: decode → process (with classification) → commit / retry / evacuate.
//! Workers supply only a per-message processing closure that returns a
//! [`ProcessOutcome`]; the runner handles backoff, the dead-letter topic, and offset
//! commits, so that logic is never copy-pasted across services.
//!
//! # Delivery semantics
//!
//! - **Success** ([`ProcessOutcome::Done`]) → the offset is committed.
//! - **Transient failure** ([`ProcessOutcome::Retry`]) → the same message is retried
//!   in place with exponential backoff and jitter, up to [`RetryPolicy::max_attempts`];
//!   on exhaustion it is dead-lettered and the offset is committed.
//! - **Permanent failure** ([`ProcessOutcome::Reject`]) or an undecodable (poison)
//!   record → dead-lettered immediately and the offset is committed.
//! - **Broker/stream error** → returned to the caller, *without* committing, so the
//!   caller can rebuild the consumer and resume from the last committed offset.
//!
//! Committing only after a terminal outcome (success or successful dead-letter) is
//! what evacuates a poison message from its partition without ever losing it: the
//! record is durably parked on the dead-letter topic before its offset advances.

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use futures_util::StreamExt;
use rand::Rng;

use crate::error::TransportError;
use crate::kafka::consumer::handle::{ConsumedMessage, KafkaConsumerHandle};
use crate::kafka::envelope::ConsumablePayload;
use crate::kafka::producer::handle::KafkaProducerHandle;

/// Suffix appended to an origin topic to form its dead-letter topic
/// (e.g. `post.published` → `post.published.dlq`). Per-origin-topic dead-letter
/// topics preserve each stream's schema and make targeted replay trivial.
/// Public so provisioning tooling (topic-provisioner) derives the exact same
/// names the runner publishes to — brokers run with auto-creation disabled.
pub const DLQ_SUFFIX: &str = ".dlq";

/// Maximum length of the human-readable error string copied into the
/// `x-dlq-error` header, to keep record headers bounded.
const MAX_DLQ_ERROR_LEN: usize = 1024;

/// Outcome of processing a single message. This is the classification surface a
/// worker maps its domain errors onto; see [`ProcessOutcome::from_result`].
#[derive(Debug)]
pub enum ProcessOutcome {
    /// Processed successfully (or intentionally skipped). Commit the offset.
    Done,
    /// Transient failure — retry with backoff, then dead-letter on exhaustion.
    /// The string is a human-readable reason recorded in the dead-letter headers.
    Retry(String),
    /// Permanent failure — dead-letter immediately without retrying. Retrying a
    /// poison payload would only stall the partition.
    Reject(String),
}

/// Lets a domain error type declare whether a failure is worth retrying. Workers
/// implement this on their error enum (transport I/O timeouts → retryable; invariant
/// violations / malformed data → not), then use [`ProcessOutcome::from_result`].
pub trait ClassifyError {
    /// `true` for transient faults that a retry might resolve; `false` for permanent
    /// (poison) failures that should be dead-lettered immediately.
    fn is_retryable(&self) -> bool;
}

impl ProcessOutcome {
    /// Maps a processing `Result` onto an outcome using the error's own
    /// [`ClassifyError`] verdict. `Ok` → [`Done`](ProcessOutcome::Done); a retryable
    /// error → [`Retry`](ProcessOutcome::Retry); otherwise
    /// [`Reject`](ProcessOutcome::Reject).
    pub fn from_result<E>(result: Result<(), E>) -> Self
    where
        E: ClassifyError + std::fmt::Display,
    {
        match result {
            Ok(()) => ProcessOutcome::Done,
            Err(e) if e.is_retryable() => ProcessOutcome::Retry(e.to_string()),
            Err(e) => ProcessOutcome::Reject(e.to_string()),
        }
    }
}

/// Bounded-retry configuration with exponential backoff and equal jitter.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Total processing attempts for a transient failure before dead-lettering.
    pub max_attempts: u32,
    /// Backoff before the first retry; doubles each subsequent attempt.
    pub base_backoff: Duration,
    /// Upper bound on a single backoff interval (before jitter).
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    /// 5 attempts, 100 ms base, 30 s cap — the validated production envelope.
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_backoff: Duration::from_millis(100),
            max_backoff:  Duration::from_secs(30),
        }
    }
}

impl RetryPolicy {
    /// Backoff before the retry that follows `attempt` (1-based) failed tries:
    /// `base · 2^(attempt-1)`, capped at `max_backoff`, then jittered uniformly into
    /// `[interval/2, interval]` (equal jitter) to de-synchronise retries across
    /// partitions during a shared-dependency outage.
    pub fn backoff_for(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31);
        let scaled   = self.base_backoff.saturating_mul(1u32 << exponent);
        let capped   = scaled.min(self.max_backoff);
        let factor: f64 = rand::rng().random_range(0.5..=1.0);
        capped.mul_f64(factor)
    }
}

/// A boxed, lifetime-bound processing future. Boxing lets the worker's closure
/// borrow both the decoded event and its own captured state (e.g. `&self`) while
/// keeping the runner generic over a single future type.
pub type ProcessFuture<'a> = Pin<Box<dyn Future<Output = ProcessOutcome> + Send + 'a>>;

/// Drives one consume cycle: decode each message, run `process` with bounded retry,
/// dead-letter terminal failures, and commit offsets after terminal outcomes.
///
/// Returns `Ok(())` only if the stream ends (it normally does not); returns `Err`
/// on a broker/stream-level fault or an unrecoverable dead-letter publish failure,
/// in both cases *without* having committed the in-flight message — so the caller's
/// restart loop resumes from the last committed offset.
///
/// # Example
///
/// ```rust,ignore
/// run_consumer::<ReactionEvent, _>(&handle, &producer, &policy, |event| {
///     Box::pin(async move { ProcessOutcome::from_result(self.process(event).await) })
/// })
/// .await?;
/// ```
pub async fn run_consumer<T, F>(
    handle:   &KafkaConsumerHandle,
    producer: &KafkaProducerHandle,
    policy:   &RetryPolicy,
    process:  F,
) -> Result<(), TransportError>
where
    T: ConsumablePayload,
    F: for<'a> Fn(&'a T) -> ProcessFuture<'a>,
{
    let mut stream = handle.stream::<T>();

    while let Some(item) = stream.next().await {
        // A broker/stream-level error carries no offset. Surface it so the caller can
        // rebuild the consumer; nothing is committed.
        let msg = item?;

        // Undecodable record → poison. Dead-letter it and commit past it.
        let event = match &msg.payload {
            Ok(event) => event,
            Err(decode_err) => {
                dead_letter(producer, &msg, "decode", &decode_err.to_string(), 0).await?;
                handle.commit(&msg)?;
                continue;
            }
        };

        // Process with bounded, jittered retry.
        let mut attempt: u32 = 1;
        loop {
            match process(event).await {
                ProcessOutcome::Done => break,
                ProcessOutcome::Reject(reason) => {
                    dead_letter(producer, &msg, "reject", &reason, attempt).await?;
                    break;
                }
                ProcessOutcome::Retry(reason) => {
                    if attempt >= policy.max_attempts {
                        dead_letter(producer, &msg, "retry-exhausted", &reason, attempt)
                            .await?;
                        break;
                    }
                    let backoff = policy.backoff_for(attempt);
                    tracing::warn!(
                        topic     = %msg.topic,
                        partition = msg.partition,
                        offset    = msg.offset,
                        attempt,
                        backoff_ms = backoff.as_millis() as u64,
                        reason    = %reason,
                        "transient processing failure — retrying after backoff"
                    );
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
            }
        }

        // Terminal outcome (success or dead-lettered) → advance the offset.
        handle.commit(&msg)?;
    }

    Ok(())
}

/// Republishes a record verbatim to its origin topic's dead-letter topic, annotated
/// with diagnostic headers. Returns `Err` if the publish fails, so the caller can
/// withhold the commit and let the record be redelivered rather than lost.
async fn dead_letter<T>(
    producer: &KafkaProducerHandle,
    msg:      &ConsumedMessage<T>,
    reason_kind: &str,
    reason:   &str,
    attempts: u32,
) -> Result<(), TransportError> {
    let dlq_topic = format!("{}{}", msg.topic, DLQ_SUFFIX);

    let truncated: String = reason.chars().take(MAX_DLQ_ERROR_LEN).collect();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let mut headers = std::collections::HashMap::new();
    headers.insert("x-dlq-origin-topic".to_string(), msg.topic.clone());
    headers.insert("x-dlq-partition".to_string(),    msg.partition.to_string());
    headers.insert("x-dlq-offset".to_string(),       msg.offset.to_string());
    headers.insert("x-dlq-reason".to_string(),       reason_kind.to_string());
    headers.insert("x-dlq-error".to_string(),        truncated);
    headers.insert("x-dlq-attempts".to_string(),     attempts.to_string());
    headers.insert("x-dlq-failed-at-ms".to_string(), now_ms.to_string());

    producer
        .publish_raw(&dlq_topic, &msg.key, &msg.raw_payload, headers)
        .await?;

    tracing::error!(
        dlq_topic = %dlq_topic,
        origin    = %msg.topic,
        partition = msg.partition,
        offset    = msg.offset,
        reason    = reason_kind,
        attempts,
        "message dead-lettered"
    );

    Ok(())
}
