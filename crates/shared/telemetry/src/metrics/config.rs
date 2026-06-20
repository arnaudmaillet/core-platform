/// Controls the metrics pipeline.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Which backend collects and exposes metrics.
    /// Defaults to [`MetricsExporterKind::Prometheus`].
    pub exporter: MetricsExporterKind,
}

/// Metrics export strategy.
#[derive(Debug, Clone, Default)]
pub enum MetricsExporterKind {
    /// Expose a Prometheus text scrape endpoint — preferred for Kubernetes.
    /// Requires the `prometheus-exporter` feature.
    #[default]
    Prometheus,
    /// Push metrics via OTLP — preferred when a central OTel Collector is deployed.
    Otlp {
        /// e.g. `http://otel-collector:4317`
        endpoint: String,
    },
}

impl MetricsConfig {
    /// Reads `METRICS_EXPORTER` (`prometheus` | `otlp`) and
    /// `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`.
    pub fn from_env() -> Self {
        let exporter = match std::env::var("METRICS_EXPORTER").as_deref() {
            Ok("otlp") => MetricsExporterKind::Otlp {
                endpoint: std::env::var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT")
                    .unwrap_or_else(|_| "http://localhost:4317".into()),
            },
            _ => MetricsExporterKind::Prometheus,
        };
        Self { exporter }
    }
}
