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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn exporter_kind_default_is_prometheus() {
        assert!(matches!(MetricsExporterKind::default(), MetricsExporterKind::Prometheus));
    }

    #[test]
    fn defaults_to_prometheus_when_no_env() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::remove_var("METRICS_EXPORTER") };
        let cfg = MetricsConfig::from_env();
        assert!(matches!(cfg.exporter, MetricsExporterKind::Prometheus));
    }

    #[test]
    fn otlp_exporter_from_env_with_default_endpoint() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::set_var("METRICS_EXPORTER", "otlp");
            std::env::remove_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT");
        }
        let cfg = MetricsConfig::from_env();
        match &cfg.exporter {
            MetricsExporterKind::Otlp { endpoint } => assert_eq!(endpoint, "http://localhost:4317"),
            other => panic!("expected Otlp, got {other:?}"),
        }
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("METRICS_EXPORTER") };
    }

    #[test]
    fn otlp_exporter_custom_endpoint() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::set_var("METRICS_EXPORTER", "otlp");
            std::env::set_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT", "http://collector:4317");
        }
        let cfg = MetricsConfig::from_env();
        match &cfg.exporter {
            MetricsExporterKind::Otlp { endpoint } => assert_eq!(endpoint, "http://collector:4317"),
            other => panic!("expected Otlp, got {other:?}"),
        }
        // SAFETY: cleanup.
        unsafe {
            std::env::remove_var("METRICS_EXPORTER");
            std::env::remove_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT");
        }
    }

    #[test]
    fn unknown_exporter_falls_back_to_prometheus() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("METRICS_EXPORTER", "unknown") };
        let cfg = MetricsConfig::from_env();
        assert!(matches!(cfg.exporter, MetricsExporterKind::Prometheus));
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("METRICS_EXPORTER") };
    }
}
