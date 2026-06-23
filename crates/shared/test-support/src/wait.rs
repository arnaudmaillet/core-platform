//! The single anti-flake primitive shared by every suite.

use std::time::Duration;

/// Polls `probe` every 50 ms until it returns `true`, or panics at `deadline`.
///
/// This is the *only* synchronisation primitive a scenario is allowed to use to
/// wait for cross-component state to converge (a Redis pub/sub round-trip, an
/// async `Drop` cleanup, a Kafka consumer-group join). Assertions wait on
/// observable state — never on a fixed sleep — so the suite is fast when the
/// system is fast and only slow when something is genuinely wrong.
///
/// `label` is surfaced in the panic message so a timeout points straight at the
/// invariant that failed to hold.
pub async fn await_until<F, Fut>(label: &str, deadline: Duration, mut probe: F)
where
    F:   FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    loop {
        if probe().await {
            return;
        }
        if start.elapsed() > deadline {
            panic!("await_until timed out after {deadline:?}: {label}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
