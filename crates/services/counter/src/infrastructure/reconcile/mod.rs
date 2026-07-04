//! The concrete reconciliation source (gRPC to `social-graph`) and the supervised
//! sweep loop that drives the [`Reconciler`](crate::application::command::Reconciler)
//! over the ledger's reconcilable pairs.

pub mod grpc_source;

pub use grpc_source::GrpcReconciliationSource;

use std::sync::Arc;
use std::time::Duration;

use crate::application::command::Reconciler;
use crate::application::port::{CounterLedger, reconcile_cursor};

/// Reconcilable pairs fetched per tick. Bounds the sweep's work; the cursor pages
/// across ticks so the whole reconcilable set is covered over time.
const RECONCILE_BATCH: i64 = 100;

/// Periodically sweeps the reconcilable `(entity, metric)` pairs, correcting any
/// exact-counter drift against the authoritative source. Pages by an opaque cursor
/// and wraps back to the start when the sweep completes. Errors are logged and the
/// sweep continues — reconciliation is best-effort background healing.
pub async fn run_reconcile_loop(
    reconciler: Arc<Reconciler>,
    ledger: Arc<dyn CounterLedger>,
    interval: Duration,
) {
    tracing::info!("counter reconcile loop started");
    let mut ticker = tokio::time::interval(interval);
    let mut cursor: Option<String> = None;
    loop {
        ticker.tick().await;

        let batch = match ledger.list_reconcilable(cursor.as_deref(), RECONCILE_BATCH).await {
            Ok(batch) => batch,
            Err(error) => {
                tracing::warn!(%error, "reconcile candidate scan failed");
                continue;
            }
        };

        if batch.is_empty() {
            cursor = None; // sweep complete — wrap to the start next tick
            continue;
        }

        if let Some((entity, metric)) = batch.last() {
            cursor = Some(reconcile_cursor(entity, *metric));
        }

        for (entity, metric) in &batch {
            if let Err(error) = reconciler.reconcile(entity, *metric).await {
                tracing::warn!(%error, metric = metric.as_str(), "reconcile failed");
            }
        }
    }
}
