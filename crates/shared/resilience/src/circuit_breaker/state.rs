use std::{sync::Arc, time::Instant};

use arc_swap::ArcSwap;
use tokio::sync::Mutex;

use super::config::CircuitBreakerConfig;

/// The three states of the circuit breaker automaton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation — requests flow through, failures are counted.
    Closed,
    /// Service is considered unavailable — requests are rejected immediately.
    Open,
    /// Cooldown expired — a limited probe request is admitted to test recovery.
    HalfOpen,
}

struct Inner {
    state: CircuitState,
    /// Timestamp set when entering Open state; used to compute the HalfOpen transition.
    open_since: Option<Instant>,
    consecutive_failures: u32,
    consecutive_successes: u32,
    /// In-flight call count while in HalfOpen (bounded by `config.half_open_max_calls`).
    half_open_inflight: u32,
}

/// Thread-safe circuit breaker state machine.
///
/// All mutable *runtime* state (counters, current state, timers) lives behind a single
/// `Mutex` so transitions are atomic (no split-brain between counters and the state enum).
///
/// The *config* is kept separately, behind an [`ArcSwap`], because it has a different
/// lifecycle: it is read-only input to transitions and can be hot-swapped by the control
/// plane without ever resetting live circuit state. Each operation samples one config
/// snapshot up-front (`load_full`) so a single call reasons against consistent thresholds.
pub struct StateMachine {
    config: Arc<ArcSwap<CircuitBreakerConfig>>,
    inner: Mutex<Inner>,
}

impl StateMachine {
    pub fn new(config: Arc<ArcSwap<CircuitBreakerConfig>>) -> Self {
        Self {
            config,
            inner: Mutex::new(Inner {
                state: CircuitState::Closed,
                open_since: None,
                consecutive_failures: 0,
                consecutive_successes: 0,
                half_open_inflight: 0,
            }),
        }
    }

    /// Returns the shared config handle so the control plane can `store()` new thresholds
    /// at runtime. Swapping the config never disturbs the live circuit state.
    pub fn config_handle(&self) -> Arc<ArcSwap<CircuitBreakerConfig>> {
        Arc::clone(&self.config)
    }

    /// Returns the current logical state, automatically driving the Open → HalfOpen
    /// transition when `open_duration` has elapsed.
    pub async fn state(&self) -> CircuitState {
        let config = self.config.load_full(); // request-scoped snapshot
        let mut inner = self.inner.lock().await;

        if inner.state == CircuitState::Open
            && inner
                .open_since
                .is_some_and(|since| since.elapsed() >= config.open_duration)
        {
            inner.state = CircuitState::HalfOpen;
            inner.consecutive_failures = 0;
            inner.consecutive_successes = 0;
            tracing::info!(prev = "Open", next = "HalfOpen", "circuit state transition");
        }

        inner.state
    }

    /// Records a successful call; drives HalfOpen → Closed when the success threshold is met.
    pub async fn on_success(&self) {
        let config = self.config.load_full(); // request-scoped snapshot
        let mut inner = self.inner.lock().await;

        inner.consecutive_failures = 0;
        inner.consecutive_successes += 1;

        if inner.state == CircuitState::HalfOpen
            && inner.consecutive_successes >= config.success_threshold
        {
            inner.state = CircuitState::Closed;
            inner.half_open_inflight = 0;
            tracing::info!(prev = "HalfOpen", next = "Closed", "circuit state transition");
        }
    }

    /// Records a failed call; drives Closed → Open on threshold, or HalfOpen → Open immediately.
    pub async fn on_failure(&self) {
        let config = self.config.load_full(); // request-scoped snapshot
        let mut inner = self.inner.lock().await;

        inner.consecutive_successes = 0;
        inner.consecutive_failures += 1;

        match inner.state {
            CircuitState::Closed if inner.consecutive_failures >= config.failure_threshold => {
                inner.state = CircuitState::Open;
                inner.open_since = Some(Instant::now());
                inner.half_open_inflight = 0;
                tracing::warn!(
                    prev = "Closed",
                    next = "Open",
                    failures = inner.consecutive_failures,
                    "circuit tripped"
                );
            }
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Open;
                inner.open_since = Some(Instant::now());
                inner.half_open_inflight = 0;
                tracing::warn!(prev = "HalfOpen", next = "Open", "probe failed — circuit re-opened");
            }
            _ => {}
        }
    }

    /// Tries to reserve a Half-Open probe slot. Returns `false` if the cap is already reached.
    pub async fn try_acquire_half_open_slot(&self) -> bool {
        let config = self.config.load_full(); // request-scoped snapshot
        let mut inner = self.inner.lock().await;

        if inner.half_open_inflight < config.half_open_max_calls {
            inner.half_open_inflight += 1;
            true
        } else {
            false
        }
    }

    /// Releases a Half-Open probe slot when the call completes (success or failure).
    pub async fn release_half_open_slot(&self) {
        let mut inner = self.inner.lock().await;
        inner.half_open_inflight = inner.half_open_inflight.saturating_sub(1);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Fast base config; transitions tested below override the relevant fields.
    fn base() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 1,
            success_threshold: 1,
            open_duration: Duration::from_millis(20),
            half_open_max_calls: 1,
        }
    }

    fn machine(config: CircuitBreakerConfig) -> StateMachine {
        StateMachine::new(Arc::new(ArcSwap::from_pointee(config)))
    }

    #[tokio::test]
    async fn trips_open_after_failure_threshold() {
        let sm = machine(CircuitBreakerConfig { failure_threshold: 3, ..base() });
        sm.on_failure().await;
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Closed, "below threshold");
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Open, "threshold reached");
    }

    #[tokio::test]
    async fn success_resets_failure_streak() {
        let sm = machine(CircuitBreakerConfig { failure_threshold: 2, ..base() });
        sm.on_failure().await;
        sm.on_success().await; // resets consecutive_failures
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn open_moves_to_half_open_after_cooldown() {
        let sm = machine(base());
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Open);
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(sm.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn half_open_closes_after_success_threshold() {
        let sm = machine(CircuitBreakerConfig {
            success_threshold: 2,
            half_open_max_calls: 5,
            ..base()
        });
        sm.on_failure().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(sm.state().await, CircuitState::HalfOpen);
        sm.on_success().await;
        assert_eq!(sm.state().await, CircuitState::HalfOpen, "one success is not enough");
        sm.on_success().await;
        assert_eq!(sm.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn half_open_reopens_on_probe_failure() {
        let sm = machine(base());
        sm.on_failure().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(sm.state().await, CircuitState::HalfOpen);
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn half_open_probe_slots_are_bounded() {
        let sm = machine(base()); // half_open_max_calls = 1
        sm.on_failure().await;
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(sm.state().await, CircuitState::HalfOpen);

        assert!(sm.try_acquire_half_open_slot().await, "first probe admitted");
        assert!(!sm.try_acquire_half_open_slot().await, "cap reached");
        sm.release_half_open_slot().await;
        assert!(sm.try_acquire_half_open_slot().await, "slot freed");
    }

    #[tokio::test]
    async fn config_hot_swap_tightens_threshold_without_resetting_state() {
        let handle = Arc::new(ArcSwap::from_pointee(CircuitBreakerConfig {
            failure_threshold: 5,
            ..base()
        }));
        let sm = StateMachine::new(Arc::clone(&handle));

        sm.on_failure().await;
        sm.on_failure().await;
        assert_eq!(sm.state().await, CircuitState::Closed);

        // Control-plane tightens the trip threshold; live failure streak is preserved.
        handle.store(Arc::new(CircuitBreakerConfig { failure_threshold: 2, ..base() }));
        sm.on_failure().await; // streak is now 3 >= new threshold 2
        assert_eq!(sm.state().await, CircuitState::Open);
    }
}
