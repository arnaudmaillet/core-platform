use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::Service;

use super::config::TimeoutConfig;
use crate::error::ResilienceError;

/// Tower [`Service`] that enforces a maximum response duration on the inner service.
pub struct TimeoutService<S> {
    inner: S,
    config: TimeoutConfig,
}

impl<S> TimeoutService<S> {
    pub(crate) fn new(inner: S, config: TimeoutConfig) -> Self {
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
        let fut = self.inner.call(req);
        let duration = self.config.duration;

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
