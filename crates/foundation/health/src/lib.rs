//! Backend health probes for readiness/liveness gating.
//!
//! [`HealthProbe`] is a graph-leaf abstraction so that **storage crates** can
//! expose ready-made probes for their clients (they only depend on this
//! foundation crate, never on the runtime), and the runtime can poll them to
//! drive a service's gRPC health status — without either side depending on the
//! other.

use std::future::Future;

use async_trait::async_trait;

/// A liveness probe the runtime polls to drive the service's gRPC health status.
///
/// Implemented by storage crates over their live clients (see each storage
/// crate's `health::probe`) or, for bespoke checks, via [`FnProbe`]. Probes must
/// be cheap — they run on every readiness tick.
#[async_trait]
pub trait HealthProbe: Send + Sync + 'static {
    /// Short identifier for logs, e.g. `"scylla"` or `"redis"`.
    fn name(&self) -> &str;

    /// Returns `Ok(())` when the dependency is reachable. Any `Err` demotes the
    /// whole service to `NOT_SERVING` until the next tick clears it.
    async fn check(&self) -> anyhow::Result<()>;
}

/// A [`HealthProbe`] backed by an async closure, for a check that isn't already
/// provided by a storage crate. The closure is `Fn` (re-run every tick),
/// typically capturing a cloned client handle:
///
/// ```ignore
/// FnProbe::new("elasticsearch", move || {
///     let client = client.clone();
///     async move { client.ping().await.map_err(Into::into) }
/// })
/// ```
pub struct FnProbe<F> {
    name: &'static str,
    check: F,
}

impl<F> FnProbe<F> {
    pub fn new(name: &'static str, check: F) -> Self {
        Self { name, check }
    }
}

#[async_trait]
impl<F, Fut> HealthProbe for FnProbe<F>
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    fn name(&self) -> &str {
        self.name
    }

    async fn check(&self) -> anyhow::Result<()> {
        (self.check)().await
    }
}
