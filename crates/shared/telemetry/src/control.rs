//! The unified, process-global telemetry control handle.

use crate::log::LogReloadHandle;
use crate::trace::SamplingHandle;

/// One handle bundling every telemetry dial — the live log filter and the trace-sampling
/// ratio. Obtain it from [`TelemetryGuard::telemetry_control`](crate::TelemetryGuard::telemetry_control)
/// and hand it to `InfraRegistry::with_telemetry_control` (with telemetry's `infra-config`
/// feature enabled) so an `infrastructure.toml` `[telemetry]` change drives both dials with
/// no redeploy. Cheap to clone; clones share the underlying reload handles.
#[derive(Clone)]
pub struct TelemetryControlHandle {
    log: LogReloadHandle,
    sampling: SamplingHandle,
}

impl TelemetryControlHandle {
    pub(crate) fn new(log: LogReloadHandle, sampling: SamplingHandle) -> Self {
        Self { log, sampling }
    }

    /// Hot-swap the log filter directly (string-in, parse-checked).
    pub fn set_log_filter(&self, directives: &str) -> Result<(), String> {
        self.log.reload(directives)
    }

    /// Hot-swap the trace-sampling ratio directly (clamped to `[0.0, 1.0]`).
    pub fn set_sampling_ratio(&self, ratio: f64) {
        self.sampling.set(ratio);
    }
}

/// Bridges the unified handle to the externalized-config layer, so an `[telemetry]` change
/// drives both dials. Gated so log/trace-only consumers never pull `infra-config` (mirrors
/// `auth-context`'s `cqrs-integration` feature).
#[cfg(feature = "infra-config")]
impl infra_config::TelemetryControl for TelemetryControlHandle {
    fn validate_filter(&self, directives: &str) -> Result<(), String> {
        self.log.validate(directives)
    }

    fn set_filter(&self, directives: &str) -> Result<(), String> {
        self.log.reload(directives)
    }

    fn set_sampling_ratio(&self, ratio: f64) {
        self.sampling.set(ratio);
    }
}
