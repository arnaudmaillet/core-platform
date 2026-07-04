use async_trait::async_trait;

use crate::domain::{EntityRef, Metric};
use crate::error::CounterError;

/// Reads the authoritative exact count from the service that owns the underlying
/// set — `engagement` for reaction-derived metrics (likes), `social-graph` for
/// follower/following.
///
/// This is what makes an *exact* metric reconcilable: the fast hot counter drifts
/// under at-least-once delivery, and the reconciliation loop periodically queries
/// the true magnitude here to correct it. A source that does not own a given
/// metric (or does not recognise the entity) returns `None`, and the reconciler
/// leaves that pair untouched.
///
/// The concrete gRPC-backed implementation is a deferred follow-up — it needs the
/// live `engagement` / `social-graph` services and a count RPC on each. This port
/// is the contract the [`Reconciler`](crate::application::command::Reconciler)
/// drives, and an in-memory fake backs its unit tests.
#[async_trait]
pub trait ReconciliationSource: Send + Sync + 'static {
    async fn authoritative_count(
        &self,
        entity: &EntityRef,
        metric: Metric,
    ) -> Result<Option<i64>, CounterError>;
}
