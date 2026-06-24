//! The aggregate registry: every externalized infrastructure section behind one
//! hot-reload target.

use std::sync::Arc;

use tracing::warn;

use crate::{
    cache::CacheRegistry, error::ConfigError, reload::Reloadable, registry::ResilienceRegistry,
    schema::InfrastructureConfig, telemetry::{LogFilterControl, TelemetrySection},
    traffic::TrafficRegistry,
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
    /// The `[telemetry]` section from boot config (the filter to apply once a control is
    /// attached). The live filter lives inside the tracing subscriber, swapped via the control.
    telemetry: Option<TelemetrySection>,
    /// Drives the process-global log filter; attached by the binary after `telemetry::init`.
    log_control: Option<Arc<dyn LogFilterControl>>,
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

        Ok(Self { resilience, cache, traffic, telemetry: config.telemetry, log_control: None })
    }

    /// Attaches the log-filter control (from `telemetry::init`) and **applies the boot
    /// `[telemetry]` filter immediately** — making the ConfigMap the source of truth over the
    /// env/default bootstrap. A bad boot filter fails loud here (caught at deploy).
    ///
    /// Call once, after `from_config` and before sharing the registry / starting the watcher.
    pub fn with_log_control(
        mut self,
        control: Arc<dyn LogFilterControl>,
    ) -> Result<Self, ConfigError> {
        if let Some(section) = &self.telemetry {
            control
                .validate_filter(&section.log_filter)
                .and_then(|()| control.set_filter(&section.log_filter))
                .map_err(ConfigError::validation)?;
        }
        self.log_control = Some(control);
        Ok(self)
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

    /// Hot-applies a freshly-parsed document to every live section (the reload entry point).
    ///
    /// Validates all sections first and bails before any mutation on failure.
    pub fn apply(&self, config: InfrastructureConfig) -> Result<(), ConfigError> {
        config.validate()?;

        // Fail-closed pre-check: the log filter is the one section whose *syntax* lives
        // behind the control, so validate it here (before any swap) to keep the reload
        // all-or-nothing — a bad directive rejects the whole document.
        if let (Some(control), Some(section)) = (&self.log_control, &config.telemetry) {
            control.validate_filter(&section.log_filter).map_err(ConfigError::validation)?;
        }

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

        // Telemetry: swap the live log filter (already validated above, so this won't fail
        // on syntax). A configured section with no control attached is a wiring gap — warn.
        match (&self.log_control, config.telemetry) {
            (Some(control), Some(section)) => {
                control.set_filter(&section.log_filter).map_err(ConfigError::validation)?;
            }
            (Some(_), None) => {
                warn!("[telemetry] section removed from reloaded config — keeping current filter")
            }
            (None, Some(_)) => {
                warn!("[telemetry] section present but no log control attached — ignored")
            }
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
