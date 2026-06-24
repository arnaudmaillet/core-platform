use tracing_subscriber::{layer::SubscriberExt, reload, util::SubscriberInitExt, EnvFilter};

use crate::{
    config::TelemetryConfig, control::TelemetryControl, error::TelemetryError, guard::TelemetryGuard,
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

    let (trace_layer, tracer_provider, sampler) = crate::trace::layer::build_trace_layer(
        &config.trace,
        &config.service_name,
        &config.service_version,
    )?;

    let metrics_pipeline = crate::metrics::layer::init_metrics_pipeline(&config.metrics)?;

    // Reloadable `EnvFilter`: base from `RUST_LOG`, else the configured default.
    // Wrapping it in `reload::Layer` yields a handle that swaps the directive live.
    let base_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.log.default_filter));
    let (filter_layer, filter_handle) = reload::Layer::new(base_filter);

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(log_layer)
        .with(trace_layer)
        .try_init()
        .map_err(|e| TelemetryError::SubscriberInit(e.to_string()))?;

    // Erase the subscriber type parameter behind a closure so the control handle
    // is nameable and storable beyond this function.
    let set_filter = Box::new(move |directive: &str| -> Result<(), TelemetryError> {
        let filter = EnvFilter::try_new(directive)
            .map_err(|e| TelemetryError::InvalidFilter(e.to_string()))?;
        filter_handle
            .reload(filter)
            .map_err(|e| TelemetryError::FilterReload(e.to_string()))
    });

    let control = TelemetryControl::new(set_filter, sampler);

    Ok(TelemetryGuard::new(log_guard, tracer_provider, metrics_pipeline, control))
}
