use std::sync::Arc;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry_sdk::metrics::SdkMeterProvider;

use super::{
    config::{MetricsConfig, MetricsExporterKind},
    exporter::PrometheusHandle,
};
use crate::error::TelemetryError;

/// Holds the running metrics pipeline and its optional Prometheus handle.
///
/// Created by [`init_metrics_pipeline`] and owned by [`crate::TelemetryGuard`].
/// Consumers access the Prometheus handle only through
/// [`crate::TelemetryGuard::prometheus_handle`].
pub(crate) struct MetricsPipeline {
    meter_provider: Option<SdkMeterProvider>,
    prometheus: Option<Arc<PrometheusHandle>>,
}

impl MetricsPipeline {
    pub(crate) fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>> {
        self.prometheus.clone()
    }

    pub(crate) fn shutdown(&self) {
        if let Some(provider) = &self.meter_provider
            && let Err(e) = provider.shutdown() {
                eprintln!("[telemetry] meter provider shutdown error: {e}");
            }
    }
}

/// Initialises the metrics pipeline according to [`MetricsConfig`].
///
/// Metrics are orthogonal to the tracing subscriber — this pipeline is
/// initialised separately and stored in [`crate::TelemetryGuard`].
/// The resulting [`SdkMeterProvider`] is also installed as the OTel global so
/// that services can call `opentelemetry::global::meter("my-service")` without
/// holding a reference to the provider.
pub(crate) fn init_metrics_pipeline(
    config: &MetricsConfig,
) -> Result<MetricsPipeline, TelemetryError> {
    match &config.exporter {
        MetricsExporterKind::Prometheus => init_prometheus_pipeline(),
        MetricsExporterKind::Otlp { endpoint } => init_otlp_pipeline(endpoint),
    }
}

// ── Prometheus ────────────────────────────────────────────────────────────────

#[cfg(feature = "prometheus-exporter")]
fn init_prometheus_pipeline() -> Result<MetricsPipeline, TelemetryError> {
    let registry = prometheus::Registry::new();

    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()
        .map_err(|e| TelemetryError::Prometheus(e.to_string()))?;

    let provider = SdkMeterProvider::builder()
        .with_reader(exporter)
        .build();

    global::set_meter_provider(provider.clone());

    Ok(MetricsPipeline {
        meter_provider: Some(provider),
        prometheus: Some(Arc::new(PrometheusHandle { registry })),
    })
}

#[cfg(not(feature = "prometheus-exporter"))]
fn init_prometheus_pipeline() -> Result<MetricsPipeline, TelemetryError> {
    Err(TelemetryError::Prometheus(
        "the `prometheus-exporter` feature is not enabled; \
         add it to your dependency or switch to MetricsExporterKind::Otlp"
            .into(),
    ))
}

// ── OTLP metrics (push via PeriodicReader) ────────────────────────────────────

fn init_otlp_pipeline(endpoint: &str) -> Result<MetricsPipeline, TelemetryError> {
    use opentelemetry_otlp::{MetricExporter, WithExportConfig};
    use opentelemetry_sdk::{metrics::PeriodicReader, runtime};

    let exporter = MetricExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()
        .map_err(|e| TelemetryError::OtlpExporter(e.to_string()))?;

    let reader = PeriodicReader::builder(exporter, runtime::Tokio)
        .with_interval(Duration::from_secs(60))
        .build();

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .build();

    global::set_meter_provider(provider.clone());

    Ok(MetricsPipeline {
        meter_provider: Some(provider),
        prometheus: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::config::{MetricsConfig, MetricsExporterKind};

    #[test]
    #[cfg(feature = "prometheus-exporter")]
    fn prometheus_pipeline_has_handle() {
        let cfg = MetricsConfig { exporter: MetricsExporterKind::Prometheus };
        let pipeline = init_metrics_pipeline(&cfg).unwrap();
        assert!(pipeline.prometheus_handle().is_some());
    }

    #[test]
    #[cfg(feature = "prometheus-exporter")]
    fn prometheus_pipeline_handle_renders_without_panic() {
        let cfg = MetricsConfig { exporter: MetricsExporterKind::Prometheus };
        let pipeline = init_metrics_pipeline(&cfg).unwrap();
        let handle = pipeline.prometheus_handle().unwrap();
        let _ = handle.render();
    }

    #[test]
    #[cfg(feature = "prometheus-exporter")]
    fn prometheus_pipeline_shutdown_does_not_panic() {
        let cfg = MetricsConfig { exporter: MetricsExporterKind::Prometheus };
        let pipeline = init_metrics_pipeline(&cfg).unwrap();
        pipeline.shutdown();
    }

}
