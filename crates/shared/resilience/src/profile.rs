//! Named resilience profiles — the bridge between externalized config and Tower layers.
//!
//! A *profile* is a fleet-meaningful class-of-service (`"standard"`, `"critical"`,
//! `"aggressive"`, …) that bundles a timeout, a circuit breaker, and a retry policy.
//!
//! Two representations, deliberately split:
//!
//! * [`ResilienceProfileSpec`] — the **wire** form. Flat, `serde`-friendly, deserialized
//!   from `infrastructure.toml` / a ConfigMap / a control-plane response by the upcoming
//!   config layer. Pure data, no live handles.
//! * [`ResilienceProfile`] — the **runtime** form. Holds the shared [`ArcSwap`] handles
//!   that the Tower layers read on every `call()`. Cloneable and cheap to pass around.
//!
//! The config layer resolves a binding (`"post-command" -> "critical"`) to a
//! `ResilienceProfileSpec`, calls [`resolve`](ResilienceProfileSpec::resolve) once at
//! boot, then builds layers from the profile. On a config change it re-parses the spec
//! and calls [`apply`](ResilienceProfile::apply) — a lock-free swap that the next request
//! observes, with zero restarts and no disturbance to in-flight futures.

use std::sync::Arc;

use arc_swap::ArcSwap;

use crate::{
    circuit_breaker::{config::CircuitBreakerConfig, layer::CircuitBreakerLayer},
    retry::{
        backoff::exponential::ExponentialBackoff,
        config::{RetryConfig, RetrySpec},
    },
    timeout::{config::TimeoutConfig, layer::TimeoutLayer},
};

/// Deserializable, fleet-facing description of one class-of-service.
///
/// ```toml
/// [resilience.profiles.critical]
/// timeout = { duration_ms = 2_000 }
/// circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30_000, half_open_max_calls = 1 }
/// retry = { max_attempts = 1, backoff = { kind = "exponential", base_ms = 20, max_ms = 500, jitter = "full" } }
/// ```
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ResilienceProfileSpec {
    pub timeout: TimeoutConfig,
    pub circuit_breaker: CircuitBreakerConfig,
    pub retry: RetrySpec,
}

impl ResilienceProfileSpec {
    /// Lowers the wire spec into a live [`ResilienceProfile`], allocating the shared
    /// `ArcSwap` handles. Call once per binding at boot.
    pub fn resolve(self) -> ResilienceProfile {
        ResilienceProfile {
            timeout: Arc::new(ArcSwap::from_pointee(self.timeout)),
            circuit_breaker: Arc::new(ArcSwap::from_pointee(self.circuit_breaker)),
            retry: self.retry.resolve(),
        }
    }
}

/// Live, layer-facing profile holding shared hot-reload handles.
///
/// Cloning is cheap (`Arc` bumps) and all clones share the same handles, so applying an
/// update through any clone reconfigures every layer built from the profile.
#[derive(Clone)]
pub struct ResilienceProfile {
    /// Hot-swappable. Shared with every [`TimeoutLayer`] built via [`timeout_layer`](Self::timeout_layer).
    pub timeout: Arc<ArcSwap<TimeoutConfig>>,
    /// Hot-swappable. Shared with every [`CircuitBreakerLayer`] built via [`circuit_breaker_layer`](Self::circuit_breaker_layer).
    pub circuit_breaker: Arc<ArcSwap<CircuitBreakerConfig>>,
    /// Resolved retry policy. Retry hot-reload is intentionally out of scope for this slice
    /// (the retry layer is generic over the backoff strategy); changing it requires rebuilding
    /// the retry layer. Timeout + circuit breaker cover the incident-critical levers.
    pub retry: RetryConfig<ExponentialBackoff>,
}

impl ResilienceProfile {
    /// Builds a [`TimeoutLayer`] bound to this profile's shared handle.
    pub fn timeout_layer(&self) -> TimeoutLayer {
        TimeoutLayer::from_handle(Arc::clone(&self.timeout))
    }

    /// Builds a [`CircuitBreakerLayer`] bound to this profile's shared handle.
    pub fn circuit_breaker_layer(&self) -> CircuitBreakerLayer {
        CircuitBreakerLayer::from_handle(Arc::clone(&self.circuit_breaker))
    }

    /// Applies a freshly-loaded spec to the live handles (the hot-reload entry point).
    ///
    /// Lock-free: each `store()` publishes a new snapshot that subsequent `call()`s pick
    /// up. In-flight requests keep the snapshot they captured at their own `call()`, so no
    /// future is torn and no semantics change mid-request. Returns the resolved retry
    /// config, which the caller must apply by rebuilding the retry layer if it changed.
    pub fn apply(&self, spec: ResilienceProfileSpec) -> RetryConfig<ExponentialBackoff> {
        self.timeout.store(Arc::new(spec.timeout));
        self.circuit_breaker.store(Arc::new(spec.circuit_breaker));
        spec.retry.resolve()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::retry::backoff::exponential::JitterKind;

    fn sample_spec() -> ResilienceProfileSpec {
        ResilienceProfileSpec {
            timeout: TimeoutConfig::from_millis(2_000),
            circuit_breaker: CircuitBreakerConfig::default(),
            retry: RetrySpec {
                max_attempts: 1,
                backoff: crate::retry::backoff::spec::BackoffSpec::Exponential {
                    base_ms: 20,
                    max_ms: 500,
                    jitter: JitterKind::Full,
                },
            },
        }
    }

    #[test]
    fn resolve_then_hot_swap_is_visible_to_new_loads() {
        let profile = sample_spec().resolve();
        assert_eq!(profile.timeout.load().duration, Duration::from_millis(2_000));
        assert_eq!(profile.retry.max_attempts, 1);

        // Simulate a control-plane push tightening the deadline + tripping point.
        let mut tighter = sample_spec();
        tighter.timeout = TimeoutConfig::from_millis(500);
        tighter.circuit_breaker.failure_threshold = 2;

        let new_retry = profile.apply(tighter);

        // A *new* load (i.e. the next request) observes the swapped values, lock-free.
        assert_eq!(profile.timeout.load().duration, Duration::from_millis(500));
        assert_eq!(profile.circuit_breaker.load().failure_threshold, 2);
        assert_eq!(new_retry.max_attempts, 1);
    }

    #[test]
    fn layers_share_the_profile_handle() {
        let profile = sample_spec().resolve();
        let layer = profile.timeout_layer();

        // The layer and the profile point at the same ArcSwap: a profile-side swap is
        // observed through the layer-side handle.
        profile.timeout.store(Arc::new(TimeoutConfig::from_millis(123)));
        assert_eq!(layer.handle().load().duration, Duration::from_millis(123));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserializes_from_toml_like_json() {
        let json = r#"{
            "timeout": { "duration_ms": 2000 },
            "circuit_breaker": {
                "failure_threshold": 5, "success_threshold": 2,
                "open_duration_ms": 30000, "half_open_max_calls": 1
            },
            "retry": {
                "max_attempts": 1,
                "backoff": { "kind": "exponential", "base_ms": 20, "max_ms": 500, "jitter": "full" }
            }
        }"#;

        let spec: ResilienceProfileSpec = serde_json::from_str(json).unwrap();
        let profile = spec.resolve();

        assert_eq!(profile.timeout.load().duration, Duration::from_millis(2_000));
        assert_eq!(profile.circuit_breaker.load().open_duration, Duration::from_secs(30));
        assert_eq!(profile.retry.backoff.max_ms, 500);
    }
}
