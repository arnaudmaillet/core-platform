use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tower::Service;

use super::state::{CircuitState, StateMachine};
use crate::error::ResilienceError;

/// Tower [`Service`] that enforces circuit-breaker semantics around an inner service.
///
/// The [`StateMachine`] is shared via [`Arc`] — all clones of this service operate
/// on the same circuit state, which is the intended behaviour for a per-dependency breaker.
/// (Generated clients such as tonic clone the service per RPC, which is exactly why the
/// state lives behind an `Arc`.)
#[derive(Clone)]
pub struct CircuitBreakerService<S> {
    inner: S,
    state_machine: Arc<StateMachine>,
}

impl<S> CircuitBreakerService<S> {
    pub(crate) fn new(inner: S, state_machine: Arc<StateMachine>) -> Self {
        Self { inner, state_machine }
    }
}

impl<S, Req> Service<Req> for CircuitBreakerService<S>
where
    S: Service<Req> + Clone + Send + 'static,
    S::Future: Send,
    // The call result is held across the `on_success` / `on_failure` state-update awaits,
    // so it must be `Send` for the boxed future to be `Send`.
    S::Response: Send,
    S::Error: Send,
    Req: Send + 'static,
{
    type Response = S::Response;
    type Error = ResilienceError<S::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Readiness is delegated to the inner service.
        // The circuit state itself is checked lazily in `call` to keep `poll_ready` non-blocking.
        self.inner.poll_ready(cx).map_err(ResilienceError::Inner)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        let sm = Arc::clone(&self.state_machine);
        // Tower's clone-in-call rule: readiness belongs to the INSTANCE the
        // caller just polled. Take that instance and leave the fresh clone
        // behind for the next poll_ready — calling a fresh clone of a
        // Buffer-backed service (tonic's Channel) panics with "`send_item`
        // called without first calling `poll_reserve`" (found live: 100% of
        // timeline→social-graph calls died under the first staging soak).
        let clone = self.inner.clone();
        let mut svc = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            // 1. Gate the request on the current circuit state.
            let mut was_half_open = false;
            match sm.state().await {
                CircuitState::Open => {
                    tracing::warn!("circuit is open, rejecting request");
                    return Err(ResilienceError::CircuitOpen);
                }
                CircuitState::HalfOpen => {
                    // Admit at most `half_open_max_calls` probes; reject the rest.
                    if !sm.try_acquire_half_open_slot().await {
                        return Err(ResilienceError::CircuitOpen);
                    }
                    was_half_open = true;
                }
                CircuitState::Closed => {}
            }

            // 2. Forward to the inner service.
            let result = svc.call(req).await;

            // 3. Record the outcome to drive state transitions.
            match &result {
                Ok(_) => sm.on_success().await,
                Err(_) => sm.on_failure().await,
            }

            // 4. Release the probe slot if we reserved one (success or failure).
            if was_half_open {
                sm.release_half_open_slot().await;
            }

            // 5. Surface the inner error, if any.
            result.map_err(ResilienceError::Inner)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use arc_swap::ArcSwap;
    use tower::{service_fn, ServiceExt};

    use super::*;
    use crate::circuit_breaker::config::CircuitBreakerConfig;

    fn machine(config: CircuitBreakerConfig) -> Arc<StateMachine> {
        Arc::new(StateMachine::new(Arc::new(ArcSwap::from_pointee(config))))
    }

    #[tokio::test]
    async fn forwards_when_closed() {
        let sm = machine(CircuitBreakerConfig::default());
        let inner = service_fn(|_: ()| async { Ok::<_, &str>("ok") });
        let mut svc = CircuitBreakerService::new(inner, sm);

        let out = svc.ready().await.unwrap().call(()).await.unwrap();
        assert_eq!(out, "ok");
    }

    #[tokio::test]
    async fn rejects_fast_once_open() {
        let cfg = CircuitBreakerConfig {
            failure_threshold: 1,
            open_duration: Duration::from_secs(30),
            ..CircuitBreakerConfig::default()
        };
        let sm = machine(cfg);
        let inner = service_fn(|_: ()| async { Err::<(), &str>("boom") });
        let mut svc = CircuitBreakerService::new(inner, sm);

        // First failure trips the circuit (threshold = 1).
        let first = svc.ready().await.unwrap().call(()).await.unwrap_err();
        assert!(matches!(first, ResilienceError::Inner("boom")));

        // Now Open: the request is rejected without touching the inner service.
        let rejected = svc.ready().await.unwrap().call(()).await.unwrap_err();
        assert!(matches!(rejected, ResilienceError::CircuitOpen));
    }

    /// Regression (staging soak finding #16): a Buffer-backed inner (tonic's
    /// Channel is one) panics "`send_item` called without first calling
    /// `poll_reserve`" if `call` runs on a fresh clone instead of the polled
    /// instance. Drives the breaker over a real `tower::buffer::Buffer` and
    /// makes many sequential calls; the clone-in-call fix keeps it from
    /// panicking.
    #[tokio::test]
    async fn survives_buffer_backed_inner_across_many_calls() {
        use tower::buffer::Buffer;

        let inner = service_fn(|_: ()| async { Ok::<_, &str>("ok") });
        let buffered = Buffer::new(inner, 8);
        let mut svc = CircuitBreakerService::new(
            buffered,
            machine(CircuitBreakerConfig::default()),
        );

        for _ in 0..25 {
            let out = svc.ready().await.unwrap().call(()).await.unwrap();
            assert_eq!(out, "ok");
        }
    }
}
