use error::AppError;

/// Decides whether a failed attempt warrants a retry.
///
/// Implementations are stateless — they inspect the error and the current attempt
/// number only. `Clone` is required so that [`RetryLayer`] can hand a policy to
/// each service clone it creates.
pub trait RetryPolicy<E>: Send + Sync + Clone + 'static {
    /// Called after each failed attempt. Returns `true` if the call should be retried.
    ///
    /// `attempt` is 1-indexed: receives `1` after the first failure.
    fn should_retry(&self, error: &E, attempt: u32) -> bool;
}

/// Default policy: delegates entirely to [`AppError::is_retryable`].
///
/// This integrates cleanly with every service error enum that already implements
/// the `AppError` contract from the `error` crate.
#[derive(Debug, Clone, Default)]
pub struct DefaultRetryPolicy;

impl<E: AppError> RetryPolicy<E> for DefaultRetryPolicy {
    fn should_retry(&self, error: &E, _attempt: u32) -> bool {
        error.is_retryable()
    }
}

/// Convenience policy that retries unconditionally, regardless of error type.
/// Useful for wrapping third-party errors that do not implement `AppError`.
#[derive(Debug, Clone, Default)]
pub struct AlwaysRetryPolicy;

impl<E> RetryPolicy<E> for AlwaysRetryPolicy {
    fn should_retry(&self, _error: &E, _attempt: u32) -> bool {
        true
    }
}

/// Policy that never retries. Useful as a no-op in tests or when disabling retry
/// at the type level without removing the middleware.
#[derive(Debug, Clone, Default)]
pub struct NeverRetryPolicy;

impl<E> RetryPolicy<E> for NeverRetryPolicy {
    fn should_retry(&self, _error: &E, _attempt: u32) -> bool {
        false
    }
}
