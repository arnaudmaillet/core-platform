//! The aggregate registry: every externalized infrastructure section behind one
//! hot-reload target.

use std::sync::Arc;

use tracing::warn;

use crate::{
    cache::CacheRegistry, error::ConfigError, reload::Reloadable, registry::ResilienceRegistry,
    schema::InfrastructureConfig, telemetry::{TelemetryControl, TelemetrySection},
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
    /// The `[telemetry]` section from boot config (the dials to apply once a control is
    /// attached). The live state lives inside the tracing subscriber / sampler, swapped via
    /// the control.
    telemetry: Option<TelemetrySection>,
    /// Drives the process-global telemetry dials; attached by the binary after `telemetry::init`.
    telemetry_control: Option<Arc<dyn TelemetryControl>>,
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

        Ok(Self {
            resilience,
            cache,
            traffic,
            telemetry: config.telemetry,
            telemetry_control: None,
        })
    }

    /// Attaches the telemetry control (from `telemetry::init`) and **applies the boot
    /// `[telemetry]` dials immediately** — making the ConfigMap the source of truth over the
    /// env/default bootstrap. A bad boot filter fails loud here (caught at deploy); the
    /// sampling range was already validated by `from_config`.
    ///
    /// Call once, after `from_config` and before sharing the registry / starting the watcher.
    pub fn with_telemetry_control(
        mut self,
        control: Arc<dyn TelemetryControl>,
    ) -> Result<Self, ConfigError> {
        if let Some(section) = &self.telemetry {
            if let Some(filter) = &section.log_filter {
                control
                    .validate_filter(filter)
                    .and_then(|()| control.set_filter(filter))
                    .map_err(ConfigError::validation)?;
            }
            if let Some(ratio) = section.sampling_ratio {
                control.set_sampling_ratio(ratio);
            }
        }
        self.telemetry_control = Some(control);
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

        // Fail-closed pre-check: the log filter is the one telemetry dial whose *syntax* lives
        // behind the control, so validate it here (before any swap) to keep the reload
        // all-or-nothing. (The sampling range was already checked by `config.validate()`.)
        if let (Some(control), Some(TelemetrySection { log_filter: Some(filter), .. })) =
            (&self.telemetry_control, &config.telemetry)
        {
            control.validate_filter(filter).map_err(ConfigError::validation)?;
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

        // Telemetry: swap the live dials. The filter was syntax-checked above and the
        // sampling range by `config.validate()`, so neither fails here. A configured section
        // with no control attached is a wiring gap — warn.
        match (&self.telemetry_control, config.telemetry) {
            (Some(control), Some(section)) => {
                if let Some(filter) = &section.log_filter {
                    control.set_filter(filter).map_err(ConfigError::validation)?;
                }
                if let Some(ratio) = section.sampling_ratio {
                    control.set_sampling_ratio(ratio);
                }
            }
            (Some(_), None) => {
                warn!("[telemetry] section removed from reloaded config — keeping current dials")
            }
            (None, Some(_)) => {
                warn!("[telemetry] section present but no telemetry control attached — ignored")
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
