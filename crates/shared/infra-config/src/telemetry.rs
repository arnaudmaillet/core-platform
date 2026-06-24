//! The `[telemetry]` section: a *singleton* (non-catalog) tenant for live log control.
//!
//! Unlike `[resilience]`/`[cache]`/`[traffic]` — which are per-key catalogs — telemetry is
//! process-global: one log-filter directive, no profiles or bindings. So this section is a
//! flat struct, and `InfraRegistry` drives it through a [`LogFilterControl`] handle rather
//! than a registry of profiles.
//!
//! ```toml
//! [telemetry]
//! log_filter = "info,post=debug,tower=warn"
//! ```

use serde::Deserialize;

use crate::error::ConfigError;

/// Drives the process-global log filter.
///
/// Implemented by the telemetry layer's reload handle (behind telemetry's `infra-config`
/// feature) and wired into [`InfraRegistry`](crate::InfraRegistry) by the serving binary.
/// Kept here, dependency-light, so `infra-config` needs no `tracing-subscriber`: filter
/// *syntax* knowledge lives entirely in the implementation.
pub trait LogFilterControl: Send + Sync {
    /// Parse-check a directive **without** applying it — used for fail-closed pre-validation
    /// so a bad filter rejects the whole reload before anything is swapped.
    fn validate_filter(&self, directives: &str) -> Result<(), String>;

    /// Parse and lock-free-swap the live filter. Implementations keep the previous filter on
    /// error, so logging can never be left broken.
    fn set_filter(&self, directives: &str) -> Result<(), String>;
}

/// The `[telemetry]` section — a singleton: one process-global log filter.
#[derive(Debug, Clone, Deserialize)]
pub struct TelemetrySection {
    /// `tracing_subscriber` `EnvFilter` directive, e.g. `"info,post=debug"`. When present,
    /// this is the source of truth at boot (overriding `RUST_LOG`/default) and the value the
    /// watcher hot-reloads.
    pub log_filter: String,
}

impl TelemetrySection {
    /// Structural validation only. Directive *syntax* is checked via
    /// [`LogFilterControl::validate_filter`] (where `tracing-subscriber` lives), so it can
    /// participate in the cross-section fail-closed pre-check without pulling that dep here.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.log_filter.trim().is_empty() {
            return Err(ConfigError::validation("[telemetry] log_filter must not be empty"));
        }
        Ok(())
    }
}
