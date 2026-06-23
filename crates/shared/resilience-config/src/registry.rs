//! Resolves the config catalog into live, hot-reloadable profiles and serves bindings.

use std::collections::HashMap;

use resilience::ResilienceProfile;
use tracing::warn;

use crate::{error::ConfigError, schema::InfrastructureConfig};

/// The boot-time-resolved set of live profiles plus the binding table.
///
/// # Topology vs. contents
///
/// The *topology* — which profiles exist and which dependency binds to which — is fixed at
/// construction. Tower layers capture a [`ResilienceProfile`]'s shared handles when they're
/// built, so re-binding a dependency would require rebuilding those layers and can't be done
/// in flight. What [`apply`](Self::apply) hot-swaps is each profile's *contents* (timeout +
/// circuit-breaker thresholds), which is the incident-critical path. Adding/removing profiles
/// or changing bindings needs a restart.
pub struct ResilienceRegistry {
    profiles: HashMap<String, ResilienceProfile>,
    bindings: HashMap<String, String>,
    default_profile: String,
}

impl ResilienceRegistry {
    /// Validates and resolves a parsed config into live profiles.
    pub fn from_config(config: InfrastructureConfig) -> Result<Self, ConfigError> {
        let section = config.resilience;
        section.validate()?;

        let profiles = section
            .profiles
            .into_iter()
            .map(|(name, spec)| (name, spec.resolve()))
            .collect();

        Ok(Self {
            profiles,
            bindings: section.bindings,
            default_profile: section.default_profile,
        })
    }

    /// Returns the live profile bound to `dependency`, falling back to the default profile.
    /// The clone is cheap (`Arc` bumps) and shares the same hot-reload handles.
    pub fn profile_for(&self, dependency: &str) -> ResilienceProfile {
        let name = self.bindings.get(dependency).unwrap_or(&self.default_profile);
        self.profiles
            .get(name)
            .or_else(|| self.profiles.get(&self.default_profile))
            .cloned()
            .expect("default_profile is guaranteed present by validate()")
    }

    /// Looks up a profile by its catalog name.
    pub fn profile(&self, name: &str) -> Option<ResilienceProfile> {
        self.profiles.get(name).cloned()
    }

    /// Hot-applies a freshly-parsed config to the live profile handles (the reload entry point).
    ///
    /// Validates first and bails before any mutation on failure, so a bad config leaves the
    /// running fleet untouched. Each matching profile's contents are lock-free-swapped; only
    /// profiles already known at boot are updated (see [topology](Self#topology-vs-contents)).
    pub fn apply(&self, config: InfrastructureConfig) -> Result<(), ConfigError> {
        let section = config.resilience;
        section.validate()?;

        for (name, profile) in &self.profiles {
            match section.profiles.get(name) {
                Some(spec) => {
                    // Retry config is returned for callers that rebuild the retry layer;
                    // timeout + circuit-breaker swaps take effect immediately.
                    let _new_retry = profile.apply(spec.clone());
                }
                None => warn!(
                    profile = %name,
                    "profile absent from reloaded config — keeping previous values (topology change requires restart)"
                ),
            }
        }

        Ok(())
    }
}
