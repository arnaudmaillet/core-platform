//! The `[cache]` section: externalized, hot-reloadable cache-TTL profiles.
//!
//! The first non-resilience tenant of the generalized config model. It mirrors the
//! resilience shape exactly — a catalog of named profiles plus per-namespace bindings —
//! so a service reads its cache TTL from a shared [`ArcSwap`] handle and an SRE can widen
//! a TTL to absorb a cache stampede *without a redeploy*.
//!
//! ```toml
//! [cache]
//! default_profile = "standard"
//!
//! [cache.profiles.standard]
//! ttl_secs = 300
//!
//! [cache.profiles.hot]
//! ttl_secs = 60
//! negative_ttl_secs = 10
//!
//! [cache.bindings]
//! "profile-view" = "standard"
//! "handle-lookup" = "hot"
//! ```

use std::{collections::HashMap, sync::Arc, time::Duration};

use arc_swap::ArcSwap;
use serde::Deserialize;
use tracing::warn;

use crate::{
    catalog::{validate_bindings, Catalog},
    error::ConfigError,
};

/// TOML header for this section, used in catalog/validation error text.
const SECTION: &str = "cache";

fn default_cache_profile() -> String {
    "standard".to_string()
}

/// The `[cache]` section: a TTL-profile catalog plus per-namespace bindings.
#[derive(Debug, Clone, Deserialize)]
pub struct CacheSection {
    /// Catalog of named cache profiles (`"standard"`, `"hot"`, …).
    pub profiles: HashMap<String, CacheProfileSpec>,

    /// Maps a cache namespace to a profile name.
    #[serde(default)]
    pub bindings: HashMap<String, String>,

    /// Profile applied to any namespace without an explicit binding.
    #[serde(default = "default_cache_profile")]
    pub default_profile: String,
}

/// Wire form of one cache class-of-service. `Duration`s serialize as flat `*_secs`
/// integers, matching the `*_ms` convention used by the resilience specs.
#[derive(Debug, Clone, Deserialize)]
pub struct CacheProfileSpec {
    /// Positive-entry TTL in seconds. Must be `> 0`.
    pub ttl_secs: u64,

    /// Negative-cache (cache-miss) TTL in seconds. `0` or absent disables negative caching.
    #[serde(default)]
    pub negative_ttl_secs: u64,
}

impl CacheProfileSpec {
    fn to_config(&self) -> CacheConfig {
        CacheConfig {
            ttl: Duration::from_secs(self.ttl_secs),
            negative_ttl: (self.negative_ttl_secs > 0)
                .then(|| Duration::from_secs(self.negative_ttl_secs)),
        }
    }

    fn resolve(&self) -> CacheProfile {
        CacheProfile {
            config: Arc::new(ArcSwap::from_pointee(self.to_config())),
        }
    }
}

/// Runtime snapshot read on the cache hot path.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Positive-entry TTL.
    pub ttl: Duration,
    /// Negative-cache TTL, when negative caching is enabled.
    pub negative_ttl: Option<Duration>,
}

/// Live, hot-reloadable cache profile. Clones are cheap and share the same handle, so a
/// reload through any clone reconfigures every caller that resolved the profile.
#[derive(Clone)]
pub struct CacheProfile {
    config: Arc<ArcSwap<CacheConfig>>,
}

impl CacheProfile {
    /// Current positive TTL. Read this per cache write so a swap takes effect on the very
    /// next entry, with no lock on the hot path.
    pub fn ttl(&self) -> Duration {
        self.config.load().ttl
    }

    /// Current negative-cache TTL, if negative caching is enabled.
    pub fn negative_ttl(&self) -> Option<Duration> {
        self.config.load().negative_ttl
    }

    /// Shared handle for callers that prefer to read the whole snapshot themselves.
    pub fn handle(&self) -> Arc<ArcSwap<CacheConfig>> {
        Arc::clone(&self.config)
    }

    /// Lock-free swap of this profile's contents (the hot-reload entry point).
    fn apply(&self, spec: &CacheProfileSpec) {
        self.config.store(Arc::new(spec.to_config()));
    }
}

impl CacheSection {
    /// Enforces the invariants the type system can't: references resolve, TTLs are positive.
    /// Run before resolving and before every hot-swap (fail-closed).
    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_bindings(SECTION, &self.profiles, &self.bindings, &self.default_profile)?;

        for (name, spec) in &self.profiles {
            if spec.ttl_secs == 0 {
                return Err(ConfigError::validation(format!(
                    "[cache] profile '{name}': ttl_secs must be > 0"
                )));
            }
        }

        Ok(())
    }
}

/// The boot-time-resolved set of live cache profiles plus the binding table.
pub struct CacheRegistry {
    catalog: Catalog<CacheProfile>,
}

impl CacheRegistry {
    /// Validates and resolves a `[cache]` section into live profiles.
    pub fn from_section(section: CacheSection) -> Result<Self, ConfigError> {
        section.validate()?;

        let profiles = section
            .profiles
            .iter()
            .map(|(name, spec)| (name.clone(), spec.resolve()))
            .collect();

        Ok(Self {
            catalog: Catalog::new(profiles, section.bindings, section.default_profile),
        })
    }

    /// Returns the live profile bound to a cache `namespace`, falling back to the default.
    pub fn profile_for(&self, namespace: &str) -> CacheProfile {
        self.catalog.profile_for(namespace)
    }

    /// Looks up a profile by catalog name.
    pub fn profile(&self, name: &str) -> Option<CacheProfile> {
        self.catalog.profile(name)
    }

    /// Hot-applies a freshly-parsed `[cache]` section to the live handles.
    ///
    /// Validates first and bails before any mutation on failure. Only profiles known at
    /// boot are swapped; a profile that disappeared from the new config keeps its previous
    /// values (topology changes require a restart).
    pub fn apply(&self, section: CacheSection) -> Result<(), ConfigError> {
        section.validate()?;

        for (name, profile) in self.catalog.iter() {
            match section.profiles.get(name) {
                Some(spec) => profile.apply(spec),
                None => warn!(
                    section = SECTION,
                    profile = %name,
                    "profile absent from reloaded config — keeping previous values (topology change requires restart)"
                ),
            }
        }

        Ok(())
    }
}
