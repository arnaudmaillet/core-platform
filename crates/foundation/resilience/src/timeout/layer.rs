use std::sync::Arc;

use arc_swap::ArcSwap;
use tower::Layer;

use super::{config::TimeoutConfig, service::TimeoutService};

/// Tower [`Layer`] that wraps an inner service with a request deadline.
///
/// The deadline lives behind a shared [`ArcSwap`] handle. Construct from a static
/// [`TimeoutConfig`] with [`new`](TimeoutLayer::new), or from an externally-owned handle
/// with [`from_handle`](TimeoutLayer::from_handle) when the config layer needs to retain
/// `store()` access for hot-reload. [`handle`](TimeoutLayer::handle) hands that access back out.
///
/// # Example
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use resilience::timeout::{TimeoutLayer, TimeoutConfig};
///
/// let layer = TimeoutLayer::new(TimeoutConfig::from_secs(5));
/// let reload = layer.handle(); // hand to the config watcher
///
/// let svc = ServiceBuilder::new().layer(layer).service(my_inner_service);
///
/// // later, from the watcher task — no restart, no rebuild:
/// reload.store(std::sync::Arc::new(TimeoutConfig::from_secs(2)));
/// ```
#[derive(Clone)]
pub struct TimeoutLayer {
    config: Arc<ArcSwap<TimeoutConfig>>,
}

impl TimeoutLayer {
    /// Builds a layer owning a fresh [`ArcSwap`] seeded with `config`.
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            config: Arc::new(ArcSwap::from_pointee(config)),
        }
    }

    /// Builds a layer that shares an externally-owned handle (e.g. one held by the
    /// config layer / [`crate::ResilienceProfile`]) so swaps propagate to this stack.
    pub fn from_handle(config: Arc<ArcSwap<TimeoutConfig>>) -> Self {
        Self { config }
    }

    /// Returns the shared handle so a control plane can `store()` new configs at runtime.
    pub fn handle(&self) -> Arc<ArcSwap<TimeoutConfig>> {
        Arc::clone(&self.config)
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = TimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimeoutService::new(inner, Arc::clone(&self.config))
    }
}
