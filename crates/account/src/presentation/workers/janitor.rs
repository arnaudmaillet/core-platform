// crates/account/src/application/workers/janitor.rs

use crate::repositories::GlobalIdentityRegistry;
use chrono::Utc;
use shared_kernel::core::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};

pub struct GlobalRegistryJanitor {
    global_registry: Arc<dyn GlobalIdentityRegistry>,
    purge_interval: Duration,
    retention_period: chrono::Duration,
}

impl GlobalRegistryJanitor {
    pub fn new(
        global_registry: Arc<dyn GlobalIdentityRegistry>,
        purge_interval: Duration,
        retention_period: chrono::Duration,
    ) -> Self {
        Self {
            global_registry,
            purge_interval,
            retention_period,
        }
    }

    /// Lance la boucle infinie du Worker (Non-blocking, à spawn dans un thread Tokio)
    pub async fn run(self) {
        info!(
            interval_secs = ?self.purge_interval.as_secs(),
            retention_mins = ?self.retention_period.num_minutes(),
            "Global Registry Janitor worker started successfully"
        );

        let mut timer = interval(self.purge_interval);

        loop {
            timer.tick().await; // Attend le prochain cycle

            if let Err(e) = self.execute_purge().await {
                error!(error = %e, "Global Registry Janitor encountered an error during purge execution");
            }
        }
    }

    async fn execute_purge(&self) -> Result<()> {
        let threshold = Utc::now() - self.retention_period;

        let purged_count = self
            .global_registry
            .purge_expired_reservations(threshold)
            .await?;

        if purged_count > 0 {
            warn!(
                purged_count,
                "Garbage Collector: Cleaned up expired PENDING global identity reservations"
            );
        }

        Ok(())
    }
}
