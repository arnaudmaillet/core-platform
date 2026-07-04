//! The `[telemetry]` section: externalized, hot-reloadable observability dials.
//!
//! Unlike the catalog sections, telemetry carries two *global* dials — the
//! log-filter directive and the trace sampling strategy — pushed to the live
//! pipeline through a [`TelemetrySink`] the serving binary registers after
//! `telemetry::init`. This crate stays free of any `telemetry`/tracing
//! dependency: the sink is a trait, and the sampling strategy is its own serde
//! enum the binary translates onto `telemetry::SamplingStrategy`.
//!
//! ```toml
//! [telemetry]
//! log_filter = "info,chat=debug"
//! sampling = { kind = "trace_id_ratio", ratio = 0.05 }
//! ```

use std::sync::{Arc, Mutex};

use arc_swap::ArcSwap;
use serde::Deserialize;

use crate::error::ConfigError;

/// The `[telemetry]` section. Both dials are optional; an absent dial leaves the
/// live value untouched on apply.
#[derive(Debug, Clone, Deserialize)]
pub struct TelemetrySection {
    /// `EnvFilter` directive for the live log filter (e.g. `"info,chat=debug"`).
    #[serde(default)]
    pub log_filter: Option<String>,
    /// Trace sampling strategy.
    #[serde(default)]
    pub sampling: Option<TelemetrySamplingSpec>,
}

/// Trace sampling strategy — infra-config's own serde mirror of
/// `telemetry::SamplingStrategy` (the binary maps between them).
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TelemetrySamplingSpec {
    /// Sample every trace.
    AlwaysOn,
    /// Drop every trace.
    AlwaysOff,
    /// Probabilistic head-based sampling; `ratio` must be in `[0.0, 1.0]`.
    TraceIdRatio { ratio: f64 },
}

impl TelemetrySection {
    /// Validates the sampling ratio range. The log-filter directive can't be
    /// validated here without a tracing dependency; it is checked at apply time
    /// by the sink (a bad directive is rejected, leaving the previous filter).
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(TelemetrySamplingSpec::TraceIdRatio { ratio }) = &self.sampling {
            if !(0.0..=1.0).contains(ratio) {
                return Err(ConfigError::validation(format!(
                    "[telemetry] sampling ratio {ratio} must be in [0.0, 1.0]"
                )));
            }
        }
        Ok(())
    }
}

/// Resolved telemetry settings pushed to the live pipeline on every apply.
#[derive(Debug, Clone, Default)]
pub struct TelemetrySettings {
    pub log_filter: Option<String>,
    pub sampling: Option<TelemetrySamplingSpec>,
}

impl From<&TelemetrySection> for TelemetrySettings {
    fn from(section: &TelemetrySection) -> Self {
        Self { log_filter: section.log_filter.clone(), sampling: section.sampling.clone() }
    }
}

/// Pushes resolved telemetry settings to the live observability pipeline.
///
/// Implemented by the serving binary over `telemetry::TelemetryControl`
/// (infra-config must not depend on the telemetry crate). Registered via
/// [`TelemetryRegistry::set_sink`] once telemetry is initialised.
pub trait TelemetrySink: Send + Sync + 'static {
    fn apply(&self, settings: &TelemetrySettings) -> Result<(), ConfigError>;
}

/// Boot-resolved telemetry settings plus the live sink, hot-reloaded together.
pub struct TelemetryRegistry {
    settings: ArcSwap<TelemetrySettings>,
    // Written once (set_sink) and read on each reload — not a hot path, so a
    // plain mutex avoids the unsized-`ArcSwapOption<dyn _>` dance.
    sink: Mutex<Option<Arc<dyn TelemetrySink>>>,
}

impl TelemetryRegistry {
    /// Validates and resolves a `[telemetry]` section into the live registry.
    pub fn from_section(section: TelemetrySection) -> Result<Self, ConfigError> {
        section.validate()?;
        Ok(Self {
            settings: ArcSwap::from_pointee(TelemetrySettings::from(&section)),
            sink: Mutex::new(None),
        })
    }

    /// The currently-resolved settings.
    pub fn settings(&self) -> Arc<TelemetrySettings> {
        self.settings.load_full()
    }

    /// Registers the live sink and immediately applies the current settings, so
    /// the boot-time `[telemetry]` values take effect as soon as telemetry is up.
    pub fn set_sink(&self, sink: Arc<dyn TelemetrySink>) -> Result<(), ConfigError> {
        sink.apply(&self.settings.load())?;
        *self.sink.lock().expect("telemetry sink mutex poisoned") = Some(sink);
        Ok(())
    }

    /// Hot-applies a reloaded `[telemetry]` section: pushes to the live sink
    /// first (fail-closed — a rejected push leaves the stored settings intact),
    /// then swaps the stored settings.
    pub fn apply(&self, section: TelemetrySection) -> Result<(), ConfigError> {
        section.validate()?;
        let settings = TelemetrySettings::from(&section);
        if let Some(sink) = self.sink.lock().expect("telemetry sink mutex poisoned").as_ref() {
            sink.apply(&settings)?;
        }
        self.settings.store(Arc::new(settings));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_out_of_range_rejected() {
        let section = TelemetrySection {
            log_filter: None,
            sampling: Some(TelemetrySamplingSpec::TraceIdRatio { ratio: 1.5 }),
        };
        assert!(section.validate().is_err());
    }

    #[test]
    fn set_sink_applies_current_settings_immediately() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Counting(Arc<AtomicUsize>);
        impl TelemetrySink for Counting {
            fn apply(&self, _: &TelemetrySettings) -> Result<(), ConfigError> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let reg = TelemetryRegistry::from_section(TelemetrySection {
            log_filter: Some("info".into()),
            sampling: None,
        })
        .unwrap();

        let calls = Arc::new(AtomicUsize::new(0));
        reg.set_sink(Arc::new(Counting(calls.clone()))).unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "boot settings applied on registration");

        reg.apply(TelemetrySection { log_filter: Some("debug".into()), sampling: None })
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2, "reload pushed to sink");
    }
}
