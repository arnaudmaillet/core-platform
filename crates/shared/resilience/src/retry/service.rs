use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::Service;

use super::{backoff::strategy::BackoffStrategy, config::RetryConfig, policy::RetryPolicy};
use crate::error::ResilienceError;

/// Tower [`Service`] that retries the inner service on transient failures.
///
/// The inner service is cloned once per `call` invocation — callers must ensure
/// `S: Clone`. This follows the standard tower pattern for services that are
/// cheap to clone (e.g. `Arc`-backed clients).
pub struct RetryService<S, P, B> {
    inner: S,
    config: RetryConfig<B>,
    policy: P,
}

impl<S, P, B> RetryService<S, P, B> {
    pub(crate) fn new(inner: S, config: RetryConfig<B>, policy: P) -> Self {
        Self { inner, config, policy }
    }
}

impl<S, Req, P, B> Service<Req> for RetryService<S, P, B>
where
    S: Service<Req> + Clone + Send + 'static,
    S::Future: Send,
    Req: Clone + Send + 'static,
    P: RetryPolicy<S::Error>,
    B: BackoffStrategy,
{
    type Response = S::Response;
    type Error = ResilienceError<S::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(ResilienceError::Inner)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let svc = self.inner.clone();
        let config = self.config.clone();
        let policy = self.policy.clone();

        Box::pin(async move {
            let mut attempt = 0u32;
            loop {
                attempt += 1;

                // Resolve each attempt fully to either a return or a backoff `delay`, so the
                // (non-`Send`) response/error never crosses the `sleep` await below — keeping
                // the boxed future `Send` without an `S::Response: Send` / `S::Error: Send` bound.
                let delay = match svc.clone().call(req.clone()).await {
                    Ok(response) => return Ok(response),
                    Err(e) => {
                        let retryable = attempt <= config.max_attempts
                            && policy.should_retry(&e, attempt);

                        if !retryable {
                            // Budget exhausted, or the policy declined to retry.
                            return if attempt > config.max_attempts {
                                Err(ResilienceError::MaxRetriesExhausted(config.max_attempts))
                            } else {
                                Err(ResilienceError::Inner(e))
                            };
                        }

                        config.backoff.next_delay(attempt)
                    }
                };

                tracing::warn!(
                    attempt,
                    max_attempts = config.max_attempts,
                    delay_ms = delay.as_millis(),
                    "transient failure — retrying"
                );
                tokio::time::sleep(delay).await;
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    use tower::{service_fn, ServiceExt};

    use super::*;
    use crate::retry::{
        backoff::exponential::{ExponentialBackoff, JitterKind},
        policy::{AlwaysRetryPolicy, NeverRetryPolicy},
    };

    /// Negligible backoff so tests don't actually sleep.
    fn fast_config(max_attempts: u32) -> RetryConfig<ExponentialBackoff> {
        RetryConfig::new(max_attempts, ExponentialBackoff::new(1, 1, JitterKind::None))
    }

    #[tokio::test]
    async fn succeeds_after_transient_failures() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);
        let inner = service_fn(move |_: ()| {
            let counter = Arc::clone(&counter);
            async move {
                // Fail the first two attempts, then succeed.
                if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err("transient")
                } else {
                    Ok::<_, &str>("ok")
                }
            }
        });
        let mut svc = RetryService::new(inner, fast_config(5), AlwaysRetryPolicy);

        let out = svc.ready().await.unwrap().call(()).await.unwrap();
        assert_eq!(out, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "2 failures + 1 success");
    }

    #[tokio::test]
    async fn exhausts_budget() {
        let inner = service_fn(|_: ()| async { Err::<(), &str>("always") });
        let mut svc = RetryService::new(inner, fast_config(2), AlwaysRetryPolicy);

        let err = svc.ready().await.unwrap().call(()).await.unwrap_err();
        assert!(matches!(err, ResilienceError::MaxRetriesExhausted(2)));
    }

    #[tokio::test]
    async fn surfaces_non_retryable_immediately() {
        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);
        let inner = service_fn(move |_: ()| {
            counter.fetch_add(1, Ordering::SeqCst);
            async { Err::<(), &str>("fatal") }
        });
        let mut svc = RetryService::new(inner, fast_config(5), NeverRetryPolicy);

        let err = svc.ready().await.unwrap().call(()).await.unwrap_err();
        assert!(matches!(err, ResilienceError::Inner("fatal")));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "no retries for a non-retryable error");
    }
}
