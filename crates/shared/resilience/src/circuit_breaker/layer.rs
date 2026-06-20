use std::sync::Arc;

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
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state_machine: Arc::new(StateMachine::new(config)),
        }
    }
}

impl<S> Layer<S> for CircuitBreakerLayer {
    type Service = CircuitBreakerService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CircuitBreakerService::new(inner, Arc::clone(&self.state_machine))
    }
}
