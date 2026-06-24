use super::backoff::{exponential::ExponentialBackoff, spec::BackoffSpec};

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

/// Deserializable, non-generic counterpart to [`RetryConfig`].
///
/// `RetryConfig<B>` is generic for zero-cost backoff dispatch and therefore can't be
/// `Deserialize`d. `RetrySpec` is what the config layer reads; [`resolve`](RetrySpec::resolve)
/// lowers it into the concrete `RetryConfig<ExponentialBackoff>` used by the layer.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RetrySpec {
    pub max_attempts: u32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub backoff: BackoffSpec,
}

impl RetrySpec {
    pub fn resolve(self) -> RetryConfig<ExponentialBackoff> {
        RetryConfig::new(self.max_attempts, self.backoff.resolve())
    }
}
