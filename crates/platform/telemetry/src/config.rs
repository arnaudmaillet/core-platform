use crate::log::config::LogConfig;
use crate::metrics::config::MetricsConfig;
use crate::trace::config::TraceConfig;

/// Root configuration for the full telemetry pipeline.
///
/// Build via [`TelemetryConfig::from_env`] (reads `RUST_LOG`, `LOG_FORMAT`,
/// `OTEL_*` env vars) or compose the sub-configs manually for tests.
///
/// # Example
///
/// ```rust,no_run
/// use telemetry::TelemetryConfig;
///
/// let cfg = TelemetryConfig::from_env("post-command-server", env!("CARGO_PKG_VERSION"));
/// let _guard = telemetry::init(cfg).expect("telemetry init failed");
/// ```
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Service name embedded in every log record, span, and metric label.
    pub service_name: String,
    /// Service version (e.g. `env!("CARGO_PKG_VERSION")`).
    pub service_version: String,
    pub log: LogConfig,
    pub trace: TraceConfig,
    pub metrics: MetricsConfig,
}

impl TelemetryConfig {
    /// Constructs the config from well-known environment variables.
    /// Absent variables fall back to safe defaults — no variable is required.
    pub fn from_env(service_name: impl Into<String>, service_version: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            service_version: service_version.into(),
            log: LogConfig::from_env(),
            trace: TraceConfig::from_env(),
            metrics: MetricsConfig::from_env(),
        }
    }
}
