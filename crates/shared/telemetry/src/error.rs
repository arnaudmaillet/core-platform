use thiserror::Error;

/// All errors that can occur during telemetry bootstrap.
#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("failed to build OTLP span exporter: {0}")]
    OtlpExporter(String),

    #[error("failed to initialise Prometheus registry: {0}")]
    Prometheus(String),

    #[error("tracing subscriber already initialised: {0}")]
    SubscriberInit(String),

    #[error("invalid sampling ratio {0}: must be in [0.0, 1.0]")]
    InvalidSamplingRatio(f64),

    #[error("invalid log filter directive: {0}")]
    InvalidFilter(String),

    #[error("failed to apply log filter reload: {0}")]
    FilterReload(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn otlp_exporter_display() {
        let e = TelemetryError::OtlpExporter("bad endpoint".into());
        assert_eq!(e.to_string(), "failed to build OTLP span exporter: bad endpoint");
    }

    #[test]
    fn prometheus_display() {
        let e = TelemetryError::Prometheus("registry failed".into());
        assert_eq!(e.to_string(), "failed to initialise Prometheus registry: registry failed");
    }

    #[test]
    fn subscriber_init_display() {
        let e = TelemetryError::SubscriberInit("already set".into());
        assert_eq!(e.to_string(), "tracing subscriber already initialised: already set");
    }

    #[test]
    fn invalid_sampling_ratio_display() {
        let e = TelemetryError::InvalidSamplingRatio(1.5);
        assert_eq!(e.to_string(), "invalid sampling ratio 1.5: must be in [0.0, 1.0]");
    }
}
