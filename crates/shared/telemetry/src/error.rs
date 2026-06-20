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
}
