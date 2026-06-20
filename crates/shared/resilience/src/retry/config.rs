use super::backoff::exponential::ExponentialBackoff;

/// Configuration for the retry middleware.
///
/// Generic over `B: BackoffStrategy` to allow injecting any delay strategy
/// without boxing (zero-cost abstraction at the type level).
#[derive(Debug, Clone)]
pub struct RetryConfig<B> {
    /// Maximum number of retry attempts, *not* counting the initial call.
    /// A value of `3` means up to 4 total attempts.
    pub max_attempts: u32,
    /// Backoff strategy used to compute the inter-attempt delay.
    pub backoff: B,
}

impl RetryConfig<ExponentialBackoff> {
    /// Sensible production default: 3 retries, exponential backoff with full jitter.
    pub fn default_exponential() -> Self {
        Self {
            max_attempts: 3,
            backoff: ExponentialBackoff::default(),
        }
    }
}

impl<B> RetryConfig<B> {
    pub fn new(max_attempts: u32, backoff: B) -> Self {
        Self { max_attempts, backoff }
    }
}
