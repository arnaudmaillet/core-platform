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
        // TODO: impl —
        //   let config = self.config.load_full(); // request-scoped snapshot
        //   lock inner;
        //   if inner.state == Open && inner.open_since.unwrap().elapsed() >= config.open_duration:
        //     inner.state = HalfOpen; inner.consecutive_failures = 0; inner.consecutive_successes = 0;
        //     info!(prev = "Open", next = "HalfOpen", "circuit state transition");
        //   return inner.state
        todo!()
    }

    /// Records a successful call; drives HalfOpen → Closed when the success threshold is met.
    pub async fn on_success(&self) {
        // TODO: impl —
        //   let config = self.config.load_full(); // request-scoped snapshot
        //   lock inner; inner.consecutive_failures = 0; inner.consecutive_successes += 1;
        //   if inner.state == HalfOpen && inner.consecutive_successes >= config.success_threshold:
        //     inner.state = Closed; inner.half_open_inflight = 0;
        //     info!(prev = "HalfOpen", next = "Closed", "circuit state transition");
        todo!()
    }

    /// Records a failed call; drives Closed → Open on threshold, or HalfOpen → Open immediately.
    pub async fn on_failure(&self) {
        // TODO: impl —
        //   let config = self.config.load_full(); // request-scoped snapshot
        //   lock inner; inner.consecutive_successes = 0; inner.consecutive_failures += 1;
        //   match inner.state:
        //     Closed if consecutive_failures >= config.failure_threshold:
        //       inner.state = Open; inner.open_since = Some(Instant::now()); inner.half_open_inflight = 0;
        //       warn!(prev = "Closed", next = "Open", failures = inner.consecutive_failures, "circuit tripped");
        //     HalfOpen:
        //       inner.state = Open; inner.open_since = Some(Instant::now()); inner.half_open_inflight = 0;
        //       warn!(prev = "HalfOpen", next = "Open", "probe failed — circuit re-opened");
        //     _ => {}
        todo!()
    }

    /// Tries to reserve a Half-Open probe slot. Returns `false` if the cap is already reached.
    pub async fn try_acquire_half_open_slot(&self) -> bool {
        // TODO: impl —
        //   let config = self.config.load_full(); // request-scoped snapshot
        //   lock inner;
        //   if inner.half_open_inflight < config.half_open_max_calls:
        //     inner.half_open_inflight += 1; return true
        //   false
        todo!()
    }

    /// Releases a Half-Open probe slot when the call completes (success or failure).
    pub async fn release_half_open_slot(&self) {
        let mut inner = self.inner.lock().await;
        inner.half_open_inflight = inner.half_open_inflight.saturating_sub(1);
    }
}
