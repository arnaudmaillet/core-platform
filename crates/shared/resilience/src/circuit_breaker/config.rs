use std::time::Duration;

/// Configuration for the circuit breaker state machine.
///
/// All thresholds and durations are tunable per-service; the defaults represent
/// conservative production values suitable for internal RPC calls.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures in Closed state required to trip the circuit (→ Open).
    pub failure_threshold: u32,
    /// Consecutive successes in Half-Open state required to reset the circuit (→ Closed).
    pub success_threshold: u32,
    /// How long the circuit stays Open before admitting a probe request (→ Half-Open).
    pub open_duration: Duration,
    /// Maximum concurrent calls allowed while in Half-Open state.
    pub half_open_max_calls: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

impl CircuitBreakerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn failure_threshold(mut self, n: u32) -> Self {
        self.failure_threshold = n;
        self
    }

    pub fn success_threshold(mut self, n: u32) -> Self {
        self.success_threshold = n;
        self
    }

    pub fn open_duration(mut self, d: Duration) -> Self {
        self.open_duration = d;
        self
    }

    pub fn half_open_max_calls(mut self, n: u32) -> Self {
        self.half_open_max_calls = n;
        self
    }
}
