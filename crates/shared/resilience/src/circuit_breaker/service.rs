use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tower::Service;

use super::state::StateMachine;
use crate::error::ResilienceError;

/// Tower [`Service`] that enforces circuit-breaker semantics around an inner service.
///
/// The [`StateMachine`] is shared via [`Arc`] — all clones of this service operate
/// on the same circuit state, which is the intended behaviour for a per-dependency breaker.
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
        let _svc = self.inner.clone();

        Box::pin(async move {
            // TODO: impl —
            //   1. match sm.state().await:
            //      CircuitState::Open → {
            //        warn!("circuit is open, rejecting request");
            //        return Err(ResilienceError::CircuitOpen);
            //      }
            //      CircuitState::HalfOpen → {
            //        if !sm.try_acquire_half_open_slot().await {
            //          return Err(ResilienceError::CircuitOpen);
            //        }
            //        // probe call — slot released in finally block below
            //      }
            //      CircuitState::Closed → {} // proceed normally
            //
            //   2. let result = _svc.call(req).await;
            //
            //   3. match &result:
            //      Ok(_) → sm.on_success().await
            //      Err(_) → sm.on_failure().await
            //
            //   4. if was HalfOpen: sm.release_half_open_slot().await
            //
            //   5. result.map_err(ResilienceError::Inner)
            drop((sm, _svc, req));
            todo!()
        })
    }
}
