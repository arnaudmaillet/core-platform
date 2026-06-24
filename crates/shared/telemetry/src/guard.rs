use std::sync::Arc;

use opentelemetry_sdk::trace::TracerProvider;
use tracing_appender::non_blocking::WorkerGuard;

use crate::control::TelemetryControlHandle;
use crate::log::LogReloadHandle;
use crate::metrics::{exporter::PrometheusHandle, layer::MetricsPipeline};
use crate::trace::SamplingHandle;

/// Lifetime anchor for the entire telemetry pipeline.
///
/// Bind the return value of [`crate::init`] to a `_guard` variable at the top
/// of `main()` and let it drop naturally when the process exits.  Dropping it
/// performs, in order:
///
/// 1. Flush and shut down the non-blocking log writer thread (`WorkerGuard`).
/// 2. Flush all in-flight OTLP spans to the remote collector and shut down
///    the [`TracerProvider`].
/// 3. Shut down the [`SdkMeterProvider`] (if metrics are enabled).
///
/// # Example
///
/// ```rust,no_run
/// #[tokio::main]
/// async fn main() {
///     let cfg = telemetry::TelemetryConfig::from_env(
///         "post-command-server",
///         env!("CARGO_PKG_VERSION"),
///     );
///     let _guard = telemetry::init(cfg).expect("telemetry init failed");
///
///     // Optionally mount the Prometheus scrape endpoint:
///     // let handle = _guard.prometheus_handle().expect("prometheus enabled");
///
///     // ... start the server ...
/// }
/// ```
pub struct TelemetryGuard {
    /// Must outlive everything; dropping this flushes buffered log records.
    _log_guard: WorkerGuard,
    /// Shutdown flushes all buffered spans to the OTLP collector.
    tracer_provider: TracerProvider,
    /// Holds the metrics pipeline; shutdown flushes the meter provider.
    metrics_pipeline: MetricsPipeline,
    /// Hot-swaps the live log filter.
    log_reloader: LogReloadHandle,
    /// Hot-swaps the live trace-sampling ratio.
    sampling_handle: SamplingHandle,
}

impl TelemetryGuard {
    pub(crate) fn new(
        log_guard: WorkerGuard,
        tracer_provider: TracerProvider,
        metrics_pipeline: MetricsPipeline,
        log_reloader: LogReloadHandle,
        sampling_handle: SamplingHandle,
    ) -> Self {
        Self {
            _log_guard: log_guard,
            tracer_provider,
            metrics_pipeline,
            log_reloader,
            sampling_handle,
        }
    }

    /// Returns the unified, cloneable [`TelemetryControlHandle`] for hot-swapping the log
    /// filter and trace-sampling ratio at runtime. Hand it to
    /// `InfraRegistry::with_telemetry_control` (with telemetry's `infra-config` feature
    /// enabled) so an `infrastructure.toml` `[telemetry]` change drives both dials with no
    /// redeploy.
    pub fn telemetry_control(&self) -> TelemetryControlHandle {
        TelemetryControlHandle::new(self.log_reloader.clone(), self.sampling_handle.clone())
    }

    /// Returns a cheaply cloneable handle to the Prometheus registry.
    ///
    /// Use this to mount a `GET /metrics` route before starting your HTTP
    /// server.  Returns `None` when the `prometheus-exporter` feature is
    /// disabled or [`MetricsExporterKind::Otlp`] is configured.
    ///
    /// [`MetricsExporterKind::Otlp`]: crate::metrics::config::MetricsExporterKind
    pub fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>> {
        self.metrics_pipeline.prometheus_handle()
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Err(e) = self.tracer_provider.shutdown() {
            eprintln!("[telemetry] tracer provider shutdown error: {e}");
        }
        self.metrics_pipeline.shutdown();
    }
}
