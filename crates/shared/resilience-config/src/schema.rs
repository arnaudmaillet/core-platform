//! Wire schema for `infrastructure.toml` and its validation.
//!
//! ```toml
//! [resilience]
//! default_profile = "standard"
//!
//! [resilience.profiles.standard]
//! timeout = { duration_ms = 10_000 }
//! circuit_breaker = { failure_threshold = 5, success_threshold = 2, open_duration_ms = 30_000, half_open_max_calls = 1 }
//! retry = { max_attempts = 3, backoff = { kind = "exponential", base_ms = 50, max_ms = 10_000, jitter = "full" } }
//!
//! [resilience.bindings]
//! "post-command"  = "critical"
//! "timeline-read" = "standard"
//! ```

use std::collections::HashMap;

use resilience::{retry::backoff::spec::BackoffSpec, ResilienceProfileSpec};
use serde::Deserialize;

use crate::error::ConfigError;

/// Top-level `infrastructure.toml` document (only the `[resilience]` section is read here).
#[derive(Debug, Clone, Deserialize)]
pub struct InfrastructureConfig {
    pub resilience: ResilienceSection,
}

/// The `[resilience]` section: a profile *catalog* plus dependency *bindings*.
#[derive(Debug, Clone, Deserialize)]
pub struct ResilienceSection {
    /// Catalog of named class-of-service profiles (`"standard"`, `"critical"`, …).
    pub profiles: HashMap<String, ResilienceProfileSpec>,

    /// Maps a downstream dependency name to a profile name.
    #[serde(default)]
    pub bindings: HashMap<String, String>,

    /// Profile applied to any dependency without an explicit binding.
    #[serde(default = "default_profile")]
    pub default_profile: String,
}

fn default_profile() -> String {
    "standard".to_string()
}

impl InfrastructureConfig {
    /// Parses a TOML document into the typed config (no validation).
    pub fn from_toml(raw: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(raw)?)
    }
}

impl ResilienceSection {
    /// Enforces the semantic invariants the type system can't: every binding and the
    /// default must reference a defined profile, and every profile's knobs must be sane.
    ///
    /// Run this before resolving *and* before every hot-swap so a malformed config can
    /// never reach the data path (fail-closed).
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.profiles.contains_key(&self.default_profile) {
            return Err(ConfigError::validation(format!(
                "default_profile '{}' is not defined under [resilience.profiles]",
                self.default_profile
            )));
        }

        for (dependency, profile) in &self.bindings {
            if !self.profiles.contains_key(profile) {
                return Err(ConfigError::validation(format!(
                    "binding '{dependency}' references unknown profile '{profile}'"
                )));
            }
        }

        for (name, spec) in &self.profiles {
            validate_profile(name, spec)?;
        }

        Ok(())
    }
}

fn validate_profile(name: &str, spec: &ResilienceProfileSpec) -> Result<(), ConfigError> {
    let err = |msg: String| ConfigError::validation(format!("profile '{name}': {msg}"));

    if spec.timeout.duration.is_zero() {
        return Err(err("timeout duration_ms must be > 0".into()));
    }

    let cb = &spec.circuit_breaker;
    if cb.failure_threshold == 0 || cb.success_threshold == 0 {
        return Err(err("failure_threshold and success_threshold must be > 0".into()));
    }
    if cb.half_open_max_calls == 0 {
        return Err(err("half_open_max_calls must be > 0".into()));
    }

    match &spec.retry.backoff {
        BackoffSpec::Exponential { base_ms, max_ms, .. } => {
            if max_ms < base_ms {
                return Err(err(format!(
                    "backoff max_ms ({max_ms}) must be >= base_ms ({base_ms})"
                )));
            }
        }
    }

    Ok(())
}
