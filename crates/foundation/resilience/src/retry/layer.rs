use tower::Layer;

use super::{config::RetryConfig, service::RetryService};

/// Tower [`Layer`] that wraps an inner service with retry logic.
///
/// # Example
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use resilience::retry::{RetryLayer, RetryConfig, DefaultRetryPolicy};
///
/// let svc = ServiceBuilder::new()
///     .layer(RetryLayer::new(RetryConfig::default_exponential(), DefaultRetryPolicy))
///     .service(my_inner_service);
/// ```
#[derive(Clone)]
pub struct RetryLayer<P, B> {
    config: RetryConfig<B>,
    policy: P,
}

impl<P, B> RetryLayer<P, B> {
    pub fn new(config: RetryConfig<B>, policy: P) -> Self {
        Self { config, policy }
    }
}

impl<S, P, B> Layer<S> for RetryLayer<P, B>
where
    P: Clone,
    B: Clone,
{
    type Service = RetryService<S, P, B>;

    fn layer(&self, inner: S) -> Self::Service {
        RetryService::new(inner, self.config.clone(), self.policy.clone())
    }
}
