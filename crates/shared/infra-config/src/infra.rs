//! The aggregate registry: every externalized infrastructure section behind one
//! hot-reload target.

use std::sync::Arc;

use tracing::warn;

use crate::{
    cache::CacheRegistry, error::ConfigError, reload::Reloadable, registry::ResilienceRegistry,
    schema::InfrastructureConfig, telemetry::TelemetryRegistry, traffic::TrafficRegistry,
};

/// Owns one resolved registry per `infrastructure.toml` section and presents them as a
/// single [`Reloadable`] for the watcher to drive.
///
/// # Fail-closed across sections
///
/// [`apply`](Self::apply) validates *every* present section before swapping *any* of them,
/// so a malformed push to one section can't partially apply across the fleet — it's
/// all-or-nothing, and a rejected reload leaves all sections on their previous values.
///
/// # Topology
///
/// Which sections exist is fixed at boot: a section present at startup hot-reloads its
/// contents; a section added or removed at runtime is ignored (logged) because callers
/// captured their handles from the sections that existed when they were wired up.
pub struct InfraRegistry {
    resilience: Arc<ResilienceRegistry>,
    cache: Option<Arc<CacheRegistry>>,
    traffic: Option<Arc<TrafficRegistry>>,
    telemetry: Option<Arc<TelemetryRegistry>>,
}

impl InfraRegistry {
    /// Validates and resolves a full config document into per-section registries.
    pub fn from_config(config: InfrastructureConfig) -> Result<Self, ConfigError> {
        // Validate the whole document up front so a bad section fails boot loudly.
        config.validate()?;

        let resilience = Arc::new(ResilienceRegistry::from_section(config.resilience)?);
        let cache = match config.cache {
            Some(section) => Some(Arc::new(CacheRegistry::from_section(section)?)),
            None => None,
        };
        let traffic = match config.traffic {
            Some(section) => Some(Arc::new(TrafficRegistry::from_section(section)?)),
            None => None,
        };
        let telemetry = match config.telemetry {
            Some(section) => Some(Arc::new(TelemetryRegistry::from_section(section)?)),
            None => None,
        };

        Ok(Self { resilience, cache, traffic, telemetry })
    }

    /// Shared resilience registry (always present).
    pub fn resilience(&self) -> Arc<ResilienceRegistry> {
        Arc::clone(&self.resilience)
    }

    /// Shared cache registry, if the deployment configured a `[cache]` section.
    pub fn cache(&self) -> Option<Arc<CacheRegistry>> {
        self.cache.clone()
    }

    /// Shared traffic (rate-limit) registry, if the deployment configured a `[traffic]` section.
    pub fn traffic(&self) -> Option<Arc<TrafficRegistry>> {
        self.traffic.clone()
    }

    /// Shared telemetry registry, if the deployment configured a `[telemetry]` section.
    /// The serving binary registers a sink on it (see [`TelemetryRegistry::set_sink`])
    /// so a config push retunes the live log filter and trace sampling.
    pub fn telemetry(&self) -> Option<Arc<TelemetryRegistry>> {
        self.telemetry.clone()
    }

    /// Hot-applies a freshly-parsed document to every live section (the reload entry point).
    ///
    /// Validates all sections first and bails before any mutation on failure.
    pub fn apply(&self, config: InfrastructureConfig) -> Result<(), ConfigError> {
        config.validate()?;

        self.resilience.apply_section(config.resilience)?;

        match (&self.cache, config.cache) {
            (Some(registry), Some(section)) => registry.apply(section)?,
            (Some(_), None) => warn!(
                "[cache] section removed from reloaded config — keeping previous values"
            ),
            (None, Some(_)) => warn!(
                "[cache] section added at runtime — ignored (adding a section requires a restart)"
            ),
            (None, None) => {}
        }

        match (&self.traffic, config.traffic) {
            (Some(registry), Some(section)) => registry.apply(section)?,
            (Some(_), None) => warn!(
                "[traffic] section removed from reloaded config — keeping previous values"
            ),
            (None, Some(_)) => warn!(
                "[traffic] section added at runtime — ignored (adding a section requires a restart)"
            ),
            (None, None) => {}
        }

        // Applied last: a bad log-filter directive can only surface here (it can't
        // be validated up front without a tracing dependency), and telemetry is
        // the one section whose apply has an external side effect (the live pipeline).
        match (&self.telemetry, config.telemetry) {
            (Some(registry), Some(section)) => registry.apply(section)?,
            (Some(_), None) => warn!(
                "[telemetry] section removed from reloaded config — keeping previous values"
            ),
            (None, Some(_)) => warn!(
                "[telemetry] section added at runtime — ignored (adding a section requires a restart)"
            ),
            (None, None) => {}
        }

        Ok(())
    }
}

impl Reloadable for InfraRegistry {
    fn reload(&self, raw: &str) -> Result<(), ConfigError> {
        self.apply(InfrastructureConfig::from_toml(raw)?)
    }
}
