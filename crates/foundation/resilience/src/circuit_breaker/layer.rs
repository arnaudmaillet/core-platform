use std::sync::Arc;

use arc_swap::ArcSwap;
use tower::Layer;

use super::{config::CircuitBreakerConfig, service::CircuitBreakerService, state::StateMachine};

/// Tower [`Layer`] that wraps an inner service with circuit-breaker protection.
///
/// A single `CircuitBreakerLayer` instance owns one [`StateMachine`] shared (via [`Arc`])
/// across every service clone it produces, ensuring a single consistent circuit state
/// per logical downstream dependency.
///
/// # Example
///
/// ```rust,ignore
/// use tower::ServiceBuilder;
/// use resilience::circuit_breaker::{CircuitBreakerLayer, CircuitBreakerConfig};
///
/// let svc = ServiceBuilder::new()
///     .layer(CircuitBreakerLayer::new(CircuitBreakerConfig::default()))
///     .service(my_inner_service);
/// ```
#[derive(Clone)]
pub struct CircuitBreakerLayer {
    state_machine: Arc<StateMachine>,
}

impl CircuitBreakerLayer {
    /// Builds a layer owning a fresh circuit, seeded with a static `config`.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self::from_handle(Arc::new(ArcSwap::from_pointee(config)))
    }

    /// Builds a layer whose thresholds are driven by an externally-owned handle (e.g. one
    /// held by [`crate::ResilienceProfile`]), so the control plane can hot-swap them.
    pub fn from_handle(config: Arc<ArcSwap<CircuitBreakerConfig>>) -> Self {
        Self {
            state_machine: Arc::new(StateMachine::new(config)),
        }
    }

    /// Returns the shared config handle so a control plane can `store()` new thresholds
    /// at runtime without resetting the live circuit state.
    pub fn handle(&self) -> Arc<ArcSwap<CircuitBreakerConfig>> {
        self.state_machine.config_handle()
    }
}

impl<S> Layer<S> for CircuitBreakerLayer {
    type Service = CircuitBreakerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CircuitBreakerService::new(inner, Arc::clone(&self.state_machine))
    }
}
