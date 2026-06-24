//! The hot-reload contract shared by every config target the watcher can drive.

use crate::error::ConfigError;

/// A hot-reload target the [`spawn_watcher`](crate::spawn_watcher) task drives with raw
/// TOML on every file change.
///
/// Implemented by [`InfraRegistry`](crate::InfraRegistry) (all sections at once) and by
/// [`ResilienceRegistry`](crate::ResilienceRegistry) (resilience only), so the same
/// single-writer `notify` watcher serves both aggregated and single-section deployments
/// without the watcher knowing any section's shape.
pub trait Reloadable: Send + Sync + 'static {
    /// Parse, validate, and lock-free-swap live config from raw TOML.
    ///
    /// **Must be fail-closed:** on any parse or validation error the implementation
    /// leaves the running config untouched and returns the error, so a bad push can
    /// never tear down the fleet.
    fn reload(&self, raw: &str) -> Result<(), ConfigError>;
}
