use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use arc_swap::ArcSwap;
use tower::Service;

use super::config::TimeoutConfig;
use crate::error::ResilienceError;

/// Tower [`Service`] that enforces a maximum response duration on the inner service.
///
/// The deadline is held behind an [`ArcSwap`] so the control plane can hot-swap it at
/// runtime: a lock-free `store()` replaces the config, and the *next* `call()` picks it
/// up. The handle is shared (via [`Arc`]) across every service clone this was layered
/// onto, so one swap reconfigures the whole stack for that downstream dependency.
///
/// `Clone` is required by stacks driving generated clients (e.g. tonic clones the service
/// per RPC); clones share the same config handle via [`Arc`].
#[derive(Clone)]
pub struct TimeoutService<S> {
    inner: S,
    config: Arc<ArcSwap<TimeoutConfig>>,
}

impl<S> TimeoutService<S> {
    pub(crate) fn new(inner: S, config: Arc<ArcSwap<TimeoutConfig>>) -> Self {
        Self { inner, config }
    }
}

impl<S, Req> Service<Req> for TimeoutService<S>
where
    S: Service<Req> + Send + 'static,
    S::Future: Send,
    Req: Send + 'static,
{
    type Response = S::Response;
    type Error = ResilienceError<S::Error>;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(ResilienceError::Inner)
    }

    fn call(&mut self, req: Req) -> Self::Future {
        // Capture the deadline snapshot once, at the very start of the invocation, so the
        // request runs against a single consistent value even if the config is swapped
        // mid-flight. `duration` is `Copy`, so the `ArcSwap` guard is dropped immediately
        // — nothing is held across the await point.
        let duration = self.config.load().duration;
        let fut = self.inner.call(req);

        Box::pin(async move {
            // TODO: impl —
            //   match tokio::time::timeout(duration, fut).await:
            //     Ok(Ok(response))  → Ok(response)
            //     Ok(Err(e))        → Err(ResilienceError::Inner(e))
            //     Err(_elapsed)     → {
            //       warn!(timeout_ms = duration.as_millis(), "request exceeded deadline");
            //       Err(ResilienceError::Timeout(duration))
            //     }
            drop((fut, duration));
            todo!()
        })
    }
}
