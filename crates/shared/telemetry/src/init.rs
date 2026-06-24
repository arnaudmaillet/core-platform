use tracing_subscriber::{layer::SubscriberExt, reload, util::SubscriberInitExt, EnvFilter};

use crate::{
    config::TelemetryConfig, error::TelemetryError, guard::TelemetryGuard, log::LogReloadHandle,
};

/// Initialises the full observability pipeline and returns a [`TelemetryGuard`].
///
/// Must be called **once**, before any `tracing::` macro is used.  The guard
/// must be kept alive until the process is ready to exit.
///
/// The layers are composed as:
/// ```text
/// Registry
///   └─ EnvFilter          (from RUST_LOG or config.log.default_filter)
///   └─ log layer          (non-blocking JSON/pretty stdout)
///   └─ trace layer        (OpenTelemetry OTLP/gRPC bridge)
/// Metrics pipeline        (Prometheus or OTLP — separate from the registry)
/// ```
///
/// # Errors
///
/// Returns [`TelemetryError`] if the OTLP exporter cannot be constructed,
/// the Prometheus registry fails, or a global subscriber is already installed.
pub fn init(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    let (log_layer, log_guard) = crate::log::layer::build_log_layer(&config.log)?;

    let (trace_layer, tracer_provider, sampling_handle) = crate::trace::layer::build_trace_layer(
        &config.trace,
        &config.service_name,
        &config.service_version,
    )?;

    let metrics_pipeline = crate::metrics::layer::init_metrics_pipeline(&config.metrics)?;

    // Bootstrap filter from RUST_LOG / default. When an `infrastructure.toml` `[telemetry]`
    // section is wired in, the binary overrides this immediately after init (ConfigMap is the
    // source of truth); absent that, this remains the live filter.
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log.default_filter));

    // Wrap the (base) filter in a reload layer so its directive can be hot-swapped at runtime.
    let (filter_layer, reload_handle) = reload::Layer::new(env_filter);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(log_layer)
        .with(trace_layer)
        .try_init()
        .map_err(|e| TelemetryError::SubscriberInit(e.to_string()))?;

    Ok(TelemetryGuard::new(
        log_guard,
        tracer_provider,
        metrics_pipeline,
        LogReloadHandle::new(reload_handle),
        sampling_handle,
    ))
}
