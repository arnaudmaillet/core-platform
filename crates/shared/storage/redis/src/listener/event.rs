use fred::error::Error as FredError;
use fred::interfaces::EventInterface;
use fred::prelude::Server;
use tokio::task::JoinHandle;

/// Spawns background Tokio tasks that forward fred connection lifecycle events
/// to the process-global `tracing` / OTel subscriber installed by the
/// `telemetry` crate.
///
/// ## Covered events
///
/// | fred callback  | tracing level | Fields                       |
/// |----------------|---------------|------------------------------|
/// | `on_reconnect` | `INFO`        | `server.host`, `server.port` |
/// | `on_error`     | `ERROR`       | `server.host`, `error`       |
///
/// ## Lifetime
///
/// The spawned tasks run until the underlying client or pool is dropped (fred
/// closes its broadcast channels at that point). There is no need to store the
/// returned handles — the Tokio runtime keeps the tasks alive for their natural
/// lifetime. They are returned for callers that need to coordinate shutdown.
///
/// ## Usage
///
/// Called automatically by [`RedisClientBuilder::build`] and
/// [`RedisPoolBuilder::build`]. You may also call it directly on any type that
/// implements `EventInterface`:
///
/// ```rust,ignore
/// use redis_storage::spawn_event_listener;
/// spawn_event_listener(&client);
/// ```
///
/// [`RedisClientBuilder::build`]: crate::client::builder::RedisClientBuilder::build
/// [`RedisPoolBuilder::build`]:   crate::pool::builder::RedisPoolBuilder::build
pub fn spawn_event_listener<C>(client: &C) -> [JoinHandle<Result<(), FredError>>; 2]
where
    C: EventInterface,
{
    // ── on_reconnect ──────────────────────────────────────────────────────────
    // fred calls this closure once per successful reconnection after a
    // connection loss. The server argument identifies which node reconnected.
    let reconnect = client.on_reconnect(|server: Server| async move {
        tracing::info!(
            server.host = %server.host,
            server.port = server.port,
            "redis client reconnected"
        );
        Ok(())
    });

    // ── on_error ──────────────────────────────────────────────────────────────
    // fred calls this closure for every connection-level error. The optional
    // `Server` identifies the specific node that errored; `None` means the
    // error is not associated with a specific node (e.g., pool-level errors).
    let error = client.on_error(|(err, server): (FredError, Option<Server>)| async move {
        let host = server
            .as_ref()
            .map(|s| s.host.to_string())
            .unwrap_or_default();
        let port = server.as_ref().map(|s| s.port).unwrap_or(0);
        tracing::error!(
            server.host  = %host,
            server.port  = port,
            error        = %err,
            "redis connection error"
        );
        Ok(())
    });

    [reconnect, error]
}
