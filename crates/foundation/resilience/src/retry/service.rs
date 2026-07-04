use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::{Service, ServiceExt};

use super::{backoff::strategy::BackoffStrategy, config::RetryConfig, policy::RetryPolicy};
use crate::error::ResilienceError;

/// Tower [`Service`] that retries the inner service on transient failures.
///
/// The inner service is taken via the clone-in-call pattern (`mem::replace`
/// with a fresh clone), and every attempt re-drives `ready()` on that same
/// instance before calling — readiness belongs to the instance the caller
/// polled, and a Buffer-backed inner (tonic's `Channel`) panics when `call`ed
/// on a clone whose slot was never reserved.
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
        // Clone-in-call rule (same fix as the circuit breaker): take the
        // instance whose readiness the caller just polled; leave the fresh
        // clone behind for the next poll_ready.
        let clone = self.inner.clone();
        let mut svc = std::mem::replace(&mut self.inner, clone);
        let config = self.config.clone();
        let policy = self.policy.clone();

        Box::pin(async move {
            let mut attempt = 0u32;
            loop {
                attempt += 1;

                // Resolve each attempt fully to either a return or a backoff `delay`, so the
                // (non-`Send`) response/error never crosses the `sleep` await below — keeping
                // the boxed future `Send` without an `S::Response: Send` / `S::Error: Send` bound.
                // Each attempt re-drives readiness on the SAME instance before calling
                // (idempotent when the slot is already held); a readiness error is an
                // attempt failure like any other. The `map(|_| ())` drops the `&mut S`
                // borrow immediately so nothing borrowed crosses the await points.
                let delay = {
                    let attempt_result = 'attempt: {
                        if let Err(e) = svc.ready().await.map(|_| ()) {
                            break 'attempt Err(e);
                        }
                        svc.call(req.clone()).await
                    };
                    // The inner block scopes `attempt_result` so the non-`Send`
                    // response/error provably drops before the sleep below.
                    match attempt_result {
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

    /// Regression (staging soak finding #16): same Buffer-backed-inner panic as
    /// the circuit breaker. The retry loop re-drives `ready()` on the taken
    /// instance each attempt; this must not panic and must still retry.
    #[tokio::test]
    async fn survives_buffer_backed_inner() {
        use tower::buffer::Buffer;

        let calls = Arc::new(AtomicU32::new(0));
        let counter = Arc::clone(&calls);
        let inner = service_fn(move |_: ()| {
            let counter = Arc::clone(&counter);
            async move {
                if counter.fetch_add(1, Ordering::SeqCst) < 2 {
                    Err("transient")
                } else {
                    Ok::<_, &str>("ok")
                }
            }
        });
        let buffered = Buffer::new(inner, 8);
        let mut svc = RetryService::new(buffered, fast_config(5), AlwaysRetryPolicy);

        let out = svc.ready().await.unwrap().call(()).await.unwrap();
        assert_eq!(out, "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3, "2 transient failures then success");
    }
}
