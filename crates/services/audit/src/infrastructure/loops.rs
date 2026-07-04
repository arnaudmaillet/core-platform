//! The worker's periodic background loops.

use std::sync::Arc;
use std::time::Duration;

use crate::application::CheckpointHandler;

/// Periodically snapshot the partition heads into a Merkle checkpoint and anchor it
/// to the external witness. The anchored root is what later lets the verifier catch
/// operator-level tampering (a regressed/rewritten head no longer matches a value
/// the operator never controlled). A transient anchor fault (`AUD-2005`) is logged
/// and retried on the next tick — chaining is unaffected.
pub async fn run_checkpoint_loop(checkpoint: Arc<CheckpointHandler>, interval: Duration) {
    tracing::info!("audit checkpoint loop started");
    let mut ticker = tokio::time::interval(interval);
    ticker.tick().await; // first tick fires immediately; nothing to anchor at t=0
    loop {
        ticker.tick().await;
        match checkpoint.create_and_anchor().await {
            Ok(cp) => tracing::debug!(head_count = cp.head_count(), "audit checkpoint anchored"),
            Err(error) => tracing::warn!(%error, "audit checkpoint anchoring failed; will retry"),
        }
    }
}
