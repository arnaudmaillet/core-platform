//! Runtime control surface for the live telemetry pipeline.
//!
//! [`TelemetryControl`] exposes the two operability dials that otherwise require
//! a redeploy: the live log-filter directive and the trace sampling rate. It is
//! produced by [`crate::init`] (via [`TelemetryGuard::control`](crate::TelemetryGuard::control))
//! and is cheaply cloneable — every clone drives the same underlying
//! `EnvFilter` reload handle and [`DynamicSampler`]. The serving runtime hands a
//! clone to the `infrastructure.toml` reload watcher so a `[telemetry]` push
//! retunes the fleet with no restart.

use std::sync::Arc;

use crate::error::TelemetryError;
use crate::trace::config::SamplingStrategy;
use crate::trace::dynamic_sampler::{sampler_for, DynamicSampler};

/// Live, cloneable handle to the telemetry dials. See the module docs.
#[derive(Clone)]
pub struct TelemetryControl {
    inner: Arc<ControlInner>,
}

/// Reloads the global `EnvFilter` from a directive string. Boxed so the
/// subscriber type parameter of the underlying reload handle stays erased.
pub(crate) type SetFilterFn = Box<dyn Fn(&str) -> Result<(), TelemetryError> + Send + Sync>;

struct ControlInner {
    set_filter: SetFilterFn,
    sampler: DynamicSampler,
}

impl TelemetryControl {
    pub(crate) fn new(
        set_filter: SetFilterFn,
        sampler: DynamicSampler,
    ) -> Self {
        Self { inner: Arc::new(ControlInner { set_filter, sampler }) }
    }

    /// Replaces the live log-filter directive (e.g. `"info,chat=debug"`).
    ///
    /// Fails (leaving the previous filter intact) if `directive` is not a valid
    /// `EnvFilter` expression.
    pub fn set_log_filter(&self, directive: &str) -> Result<(), TelemetryError> {
        (self.inner.set_filter)(directive)
    }

    /// Retunes trace sampling live. Ratio strategies are applied parent-based, so
    /// distributed traces stay whole.
    pub fn set_sampling(&self, strategy: SamplingStrategy) -> Result<(), TelemetryError> {
        self.inner.sampler.set(sampler_for(&strategy)?);
        Ok(())
    }
}

impl std::fmt::Debug for TelemetryControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TelemetryControl")
    }
}
