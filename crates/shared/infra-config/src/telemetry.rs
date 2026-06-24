//! The `[telemetry]` section: a *singleton* (non-catalog) tenant for live observability control.
//!
//! Unlike `[resilience]`/`[cache]`/`[traffic]` — which are per-key catalogs — telemetry is
//! process-global: one log-filter directive and one trace-sampling ratio, no profiles or
//! bindings. So this section is a flat struct, and `InfraRegistry` drives it through a single
//! [`TelemetryControl`] handle rather than a registry of profiles.
//!
//! ```toml
//! [telemetry]
//! log_filter     = "info,post=debug,tower=warn"
//! sampling_ratio = 0.1   # 0.0 = off, 1.0 = all, mid = head-based ratio
//! ```

use serde::Deserialize;

use crate::error::ConfigError;

/// Drives the process-global telemetry dials.
///
/// Implemented by the telemetry layer's control handle (behind telemetry's `infra-config`
/// feature) and wired into [`InfraRegistry`](crate::InfraRegistry) by the serving binary.
/// Kept here, dependency-light, so `infra-config` needs no `tracing-subscriber`/OTel: the
/// log-filter *syntax* lives in the implementation, while the sampling *range* is a plain
/// number validated here (see [`TelemetrySection::validate`]).
pub trait TelemetryControl: Send + Sync {
    /// Parse-check a log-filter directive **without** applying it — used for fail-closed
    /// pre-validation so a bad filter rejects the whole reload before anything is swapped.
    fn validate_filter(&self, directives: &str) -> Result<(), String>;

    /// Parse and lock-free-swap the live log filter. Implementations keep the previous filter
    /// on error, so logging is never left broken.
    fn set_filter(&self, directives: &str) -> Result<(), String>;

    /// Lock-free-swap the live trace-sampling ratio. Infallible: the `[0.0, 1.0]` range is
    /// validated upstream by [`TelemetrySection::validate`] (and clamped defensively).
    fn set_sampling_ratio(&self, ratio: f64);
}

/// The `[telemetry]` section — a singleton. Both dials are optional, so a deployment can
/// control either, both, or neither.
#[derive(Debug, Clone, Deserialize)]
pub struct TelemetrySection {
    /// `tracing_subscriber` `EnvFilter` directive, e.g. `"info,post=debug"`. When present,
    /// this is the boot source of truth (over `RUST_LOG`/default) and the value hot-reloaded.
    #[serde(default)]
    pub log_filter: Option<String>,

    /// Head-based trace-sampling ratio in `[0.0, 1.0]` (`0.0` = off, `1.0` = all). When
    /// present, the boot source of truth (over the env sampler) and the value hot-reloaded.
    #[serde(default)]
    pub sampling_ratio: Option<f64>,
}

impl TelemetrySection {
    /// Validates both dials: a present `log_filter` must be non-empty (syntax is checked via
    /// [`TelemetryControl::validate_filter`] where `tracing-subscriber` lives); a present
    /// `sampling_ratio` must be in `[0.0, 1.0]`. Run in the cross-section fail-closed
    /// pre-check, so a bad value rejects the whole document before any swap.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.log_filter.as_deref().is_some_and(|f| f.trim().is_empty()) {
            return Err(ConfigError::validation("[telemetry] log_filter must not be empty"));
        }
        if let Some(ratio) = self.sampling_ratio.filter(|r| !(0.0..=1.0).contains(r)) {
            return Err(ConfigError::validation(format!(
                "[telemetry] sampling_ratio {ratio} must be in [0.0, 1.0]"
            )));
        }
        Ok(())
    }
}
