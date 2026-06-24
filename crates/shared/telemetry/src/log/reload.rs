//! Runtime hot-swap of the global log-filter directive.
//!
//! The `EnvFilter` is installed as the base layer on the `Registry`, so its reload handle
//! has a *nameable* type (`reload::Handle<EnvFilter, Registry>`) — no type erasure needed.
//! [`LogReloadHandle`] wraps it with parse-checked, string-in/string-out methods so callers
//! (and the externalized config layer) never touch `tracing-subscriber` types directly.

use tracing_subscriber::{reload, EnvFilter, Registry};

/// Cheap, cloneable handle to hot-swap the process-global log filter at runtime.
///
/// Obtain it from [`TelemetryGuard::log_reloader`](crate::TelemetryGuard::log_reloader).
#[derive(Clone)]
pub struct LogReloadHandle {
    handle: reload::Handle<EnvFilter, Registry>,
}

impl LogReloadHandle {
    pub(crate) fn new(handle: reload::Handle<EnvFilter, Registry>) -> Self {
        Self { handle }
    }

    /// Parse-check a directive (e.g. `"info,post=debug"`) without applying it.
    pub fn validate(&self, directives: &str) -> Result<(), String> {
        EnvFilter::try_new(directives).map(|_| ()).map_err(|e| e.to_string())
    }

    /// Parse and lock-free-swap the live filter. On a parse error the previous filter is
    /// kept, so logging is never left broken.
    pub fn reload(&self, directives: &str) -> Result<(), String> {
        let filter = EnvFilter::try_new(directives).map_err(|e| e.to_string())?;
        self.handle.reload(filter).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::layer::SubscriberExt;

    /// A `LogReloadHandle` over a local (non-global) subscriber. The subscriber is returned
    /// so it stays alive — the handle swaps the filter inside it.
    fn handle() -> (impl tracing::Subscriber, LogReloadHandle) {
        let (layer, h) = reload::Layer::new(EnvFilter::new("info"));
        let subscriber = tracing_subscriber::registry().with(layer);
        (subscriber, LogReloadHandle::new(h))
    }

    #[test]
    fn validate_accepts_good_and_rejects_bad() {
        let (_sub, h) = handle();
        assert!(h.validate("info,post=debug,tower=warn").is_ok());
        assert!(h.validate("post=notalevel").is_err());
    }

    #[test]
    fn reload_swaps_good_and_keeps_previous_on_bad() {
        let (_sub, h) = handle();
        assert!(h.reload("debug,post=trace").is_ok());
        // A malformed directive is rejected; the previous filter stays live.
        assert!(h.reload("post=notalevel").is_err());
    }
}
