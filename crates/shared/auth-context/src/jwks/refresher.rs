use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time;

use super::{JwksCache, JwksClient};

/// Background Tokio task that keeps the [`JwksCache`] populated and fresh.
///
/// ## Lifecycle
///
/// 1. [`JwksRefresher::spawn`] immediately schedules the first fetch so that
///    the cache is warm before the first HTTP request arrives at the service.
/// 2. After each successful fetch the task sleeps for `refresh_interval`.
/// 3. On failure the task applies exponential backoff starting at 1 s, doubling
///    on each consecutive error, capped at `max_backoff`. The stale key set
///    remains in the cache throughout — requests continue to be served until
///    the IdP is reachable again.
/// 4. Call [`JwksRefresher::shutdown`] during graceful process shutdown.
///    The Tokio abort is clean because the task holds no `Drop`-sensitive
///    resources (the `JwksClient` HTTP connection pool is owned by `reqwest`).
///
/// ## Observability
///
/// Every successful refresh emits `tracing::info!` with the `key_count` field.
/// Every failed refresh emits `tracing::warn!` with the error and next
/// retry interval, enabling alert rules on repeated WARN bursts.
pub struct JwksRefresher {
    handle: JoinHandle<()>,
}

impl JwksRefresher {
    /// Spawns the background refresh task and returns immediately.
    ///
    /// The caller must hold the returned [`JwksRefresher`] for the full
    /// lifetime of the process. Dropping it does **not** shut down the task
    /// (the `JoinHandle` is detached on drop). Call [`shutdown`] explicitly.
    ///
    /// [`shutdown`]: JwksRefresher::shutdown
    pub fn spawn(
        client: JwksClient,
        cache: JwksCache,
        refresh_interval: Duration,
        max_backoff: Duration,
    ) -> Self {
        let handle = tokio::spawn(Self::run(client, cache, refresh_interval, max_backoff));
        Self { handle }
    }

    /// Aborts the background refresh task.
    ///
    /// Returns immediately; any in-flight JWKS fetch is cancelled.
    pub fn shutdown(self) {
        self.handle.abort();
    }

    async fn run(
        client: JwksClient,
        cache: JwksCache,
        refresh_interval: Duration,
        max_backoff: Duration,
    ) {
        let mut backoff = Duration::from_secs(1);
        // First iteration fires with zero delay so the cache is immediately warm.
        let mut delay = Duration::ZERO;

        loop {
            time::sleep(delay).await;

            match client.fetch().await {
                Ok(keys) => {
                    let count = keys.len();
                    cache.replace(keys).await;

                    tracing::info!(
                        key_count = count,
                        next_refresh_secs = refresh_interval.as_secs(),
                        "JWKS cache refreshed successfully"
                    );

                    backoff = Duration::from_secs(1);
                    delay = refresh_interval;
                }

                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        retry_after_secs = backoff.as_secs(),
                        "JWKS refresh failed; serving stale keys until recovered"
                    );
                    delay = backoff;
                    backoff = (backoff * 2).min(max_backoff);
                }
            }
        }
    }
}
