//! The reusable "named-profile catalog + dependency bindings" shape.
//!
//! Every externalized section (resilience, cache, …) is a *catalog* of named
//! class-of-service profiles plus a table binding each dependency/namespace to a profile
//! by name, with a default for the unbound. The reference-integrity check and the
//! resolved lookup table are identical across sections, so they live here once.
//!
//! # Topology vs. contents
//!
//! The *topology* — which profiles exist and which dependency binds to which — is fixed at
//! construction: callers capture a live profile's shared handle when they wire themselves
//! up, so re-binding can't happen in flight. What hot-reload swaps is each profile's
//! *contents*. Adding/removing profiles or changing bindings needs a restart.

use std::collections::HashMap;

use crate::error::ConfigError;

/// Validates that `default` and every binding target names a profile present in `profiles`.
///
/// Shared by every catalog-shaped section so the fail-closed reference check exists in one
/// place; `section` is the TOML header (`"resilience"`, `"cache"`, …) used in error text.
pub(crate) fn validate_bindings<S>(
    section: &str,
    profiles: &HashMap<String, S>,
    bindings: &HashMap<String, String>,
    default: &str,
) -> Result<(), ConfigError> {
    if !profiles.contains_key(default) {
        return Err(ConfigError::validation(format!(
            "[{section}] default_profile '{default}' is not defined under [{section}.profiles]"
        )));
    }

    for (dependency, profile) in bindings {
        if !profiles.contains_key(profile) {
            return Err(ConfigError::validation(format!(
                "[{section}] binding '{dependency}' references unknown profile '{profile}'"
            )));
        }
    }

    Ok(())
}

/// A resolved catalog: named *live* profiles + the binding table + the default name.
///
/// `L` is the runtime (handle-bearing) profile type for a section. Cloning a resolved
/// profile is cheap (`Arc` bumps) and every clone shares the same hot-reload handle.
#[derive(Clone)]
pub struct Catalog<L> {
    profiles: HashMap<String, L>,
    bindings: HashMap<String, String>,
    default: String,
}

impl<L: Clone> Catalog<L> {
    /// Builds a catalog from already-resolved live profiles. Callers must have validated
    /// references first (via [`validate_bindings`]); `profile_for` relies on the default
    /// being present.
    pub(crate) fn new(
        profiles: HashMap<String, L>,
        bindings: HashMap<String, String>,
        default: String,
    ) -> Self {
        Self { profiles, bindings, default }
    }

    /// Returns the live profile bound to `dependency`, falling back to the default profile.
    pub fn profile_for(&self, dependency: &str) -> L {
        let name = self.bindings.get(dependency).unwrap_or(&self.default);
        self.profiles
            .get(name)
            .or_else(|| self.profiles.get(&self.default))
            .cloned()
            .expect("default profile is guaranteed present by validate_bindings()")
    }

    /// Looks up a live profile by its catalog name.
    pub fn profile(&self, name: &str) -> Option<L> {
        self.profiles.get(name).cloned()
    }

    /// Iterates resolved `(name, live)` pairs — used by `apply` to swap contents in place
    /// for profiles that survived a reload.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &L)> {
        self.profiles.iter()
    }
}
