use tower::Layer;

use super::{config::TimeoutConfig, service::TimeoutService};

/// Tower [`Layer`] that wraps an inner service with a request deadline.
///
/// # Example
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use resilience::timeout::{TimeoutLayer, TimeoutConfig};
///
/// let svc = ServiceBuilder::new()
///     .layer(TimeoutLayer::new(TimeoutConfig::from_secs(5)))
///     .service(my_inner_service);
/// ```
#[derive(Clone, Copy)]
pub struct TimeoutLayer {
    config: TimeoutConfig,
}

impl TimeoutLayer {
    pub fn new(config: TimeoutConfig) -> Self {
        Self { config }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = TimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimeoutService::new(inner, self.config)
    }
}
