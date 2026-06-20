use std::sync::Arc;

use scylla::client::caching_session::{CachingSession, CachingSessionBuilder};
use scylla::client::session_builder::SessionBuilder;
use scylla::frame::Compression;

use crate::config::ScyllaConfig;
use crate::config::cluster::CompressionKind;
use crate::error::ScyllaStorageError;
use crate::listener::OtelHistoryListener;
use crate::profile::ProfileRegistry;

/// A fully-initialised ScyllaDB client bundled with its supporting
/// infrastructure.
///
/// Produced exclusively by [`ScyllaSessionBuilder::build`]. Consumers hold an
/// `Arc<ScyllaClient>` and share it across CQRS command/query handlers.
///
/// ## Usage pattern
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use scylla::observability::history::HistoryListener;
/// use scylla::statement::unprepared::Statement;
///
/// let client = Arc::new(ScyllaSessionBuilder::new(config).build().await?);
///
/// // CQRS write handler — attach the shared listener before execution.
/// let mut stmt = Statement::new("INSERT INTO feed.events (id, ts) VALUES (?, ?)");
/// stmt.set_history_listener(Arc::clone(&client.history_listener) as Arc<dyn HistoryListener>);
/// stmt.set_execution_profile_handle(
///     client.profiles.get(ProfileKind::Strict).clone().into_handle(),
/// );
/// client.session.query_unpaged(stmt, (id, ts)).await?;
/// ```
pub struct ScyllaClient {
    /// Token-aware session with a prepared-statement LRU cache.
    pub session: CachingSession,

    /// Per-statement OTel tracing bridge.
    ///
    /// Clone the `Arc` and attach to each statement before execution —
    /// the same instance is safe to share across all statements.
    pub history_listener: Arc<OtelHistoryListener>,

    /// Pre-built execution profiles.
    ///
    /// Select the appropriate profile for each statement:
    /// - [`ProfileKind::Strict`]     — mutations requiring `LocalQuorum`
    /// - [`ProfileKind::Fast`]       — latency-sensitive reads (`LocalOne` + speculative)
    /// - [`ProfileKind::Analytical`] — background jobs (`Quorum`, 30 s timeout)
    pub profiles: ProfileRegistry,
}

/// Orchestrates ScyllaDB session construction from a [`ScyllaConfig`].
///
/// Applies the following defaults in order:
/// 1. Registers the `Strict` profile as the session-level default.
/// 2. Wraps the raw `Session` in a `CachingSession` for token-aware
///    prepared-statement routing and automatic re-prepare on schema change.
/// 3. Wires the shared `OtelHistoryListener` to the returned client (callers
///    attach it per-statement).
///
/// ## Example
///
/// ```rust,ignore
/// let config = ScyllaConfig::from_env()?;
/// let client = ScyllaSessionBuilder::new(config).build().await?;
/// ```
pub struct ScyllaSessionBuilder {
    config: ScyllaConfig,
}

impl ScyllaSessionBuilder {
    pub fn new(config: ScyllaConfig) -> Self {
        Self { config }
    }

    /// Connects to the cluster and returns a [`ScyllaClient`].
    ///
    /// ## Errors
    ///
    /// Returns [`ScyllaStorageError::Bootstrap`] when no contact point is
    /// reachable. Returns [`ScyllaStorageError::Configuration`] when the
    /// provided `ScyllaConfig` is semantically invalid (empty contact points,
    /// unknown keyspace, unknown local DC).
    pub async fn build(self) -> Result<ScyllaClient, ScyllaStorageError> {
        let cfg = &self.config;

        let profiles = ProfileRegistry::new(cfg.local_dc.clone());

        let compression = match cfg.compression {
            CompressionKind::None    => None,
            CompressionKind::Lz4     => Some(Compression::Lz4),
            CompressionKind::Snappy  => Some(Compression::Snappy),
        };

        let mut builder = SessionBuilder::new()
            .known_nodes(cfg.contact_points.iter().map(String::as_str))
            .connection_timeout(cfg.connect_timeout)
            .compression(compression)
            .default_execution_profile_handle(
                profiles.strict().clone().into_handle_with_label("strict".to_string()),
            );

        if let Some(ks) = &cfg.keyspace {
            builder = builder.use_keyspace(ks.clone(), false);
        }

        if let (Some(user), Some(pass)) = (&cfg.username, &cfg.password) {
            builder = builder.user(user.clone(), pass.clone());
        }

        let session = builder
            .build()
            .await
            .map_err(ScyllaStorageError::from)?;

        let caching_session = CachingSessionBuilder::new(session)
            .max_capacity(cfg.statement_cache_capacity)
            .build();

        Ok(ScyllaClient {
            session:          caching_session,
            history_listener: OtelHistoryListener::arc(),
            profiles,
        })
    }
}

