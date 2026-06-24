use std::time::Duration;

/// Computes the delay to apply before the next retry attempt.
///
/// `attempt` is 1-indexed: the first retry receives `attempt = 1`.
/// Implementations must be `Clone` so [`RetryLayer`] can hand one to each service clone.
pub trait BackoffStrategy: Send + Sync + Clone + 'static {
    fn next_delay(&self, attempt: u32) -> Duration;
}
