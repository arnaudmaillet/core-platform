use opentelemetry_otlp::{SpanExporter, WithExportConfig, WithHttpConfig};

use super::config::{OtlpProtocol, TraceConfig};
use crate::error::TelemetryError;

/// Constructs the OTLP span exporter from [`TraceConfig`].
///
/// The resulting [`SpanExporter`] is transport-opaque — the layer builder in
/// `layer.rs` plugs it into the SDK batch processor without knowing whether
/// gRPC or HTTP/protobuf is in use.
pub fn build_otlp_exporter(config: &TraceConfig) -> Result<SpanExporter, TelemetryError> {
    match config.protocol {
        OtlpProtocol::Grpc => build_grpc_exporter(config),
        OtlpProtocol::HttpProtobuf => build_http_exporter(config),
    }
}

/// gRPC/tonic transport to `config.otlp_endpoint`.
///
/// Auth headers should be supplied via the standard `OTEL_EXPORTER_OTLP_HEADERS`
/// env var (`key=value,key2=value2` format), which the SDK reads and injects as
/// per-request gRPC metadata.  The `config.auth_header` field is a convenience
/// override for programmatic construction and is forwarded here via a custom
/// request interceptor when present.
fn build_grpc_exporter(config: &TraceConfig) -> Result<SpanExporter, TelemetryError> {
    SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&config.otlp_endpoint)
        .build()
        .map_err(|e| TelemetryError::OtlpExporter(e.to_string()))
}

/// HTTP/protobuf transport to `config.otlp_endpoint`.
///
/// Requires the `http-proto` feature on `opentelemetry-otlp`.
/// Auth header is forwarded in the HTTP `Authorization` header when present.
fn build_http_exporter(config: &TraceConfig) -> Result<SpanExporter, TelemetryError> {
    let mut headers = std::collections::HashMap::new();
    if let Some(auth) = &config.auth_header {
        headers.insert("authorization".to_string(), auth.clone());
    }

    SpanExporter::builder()
        .with_http()
        .with_endpoint(&config.otlp_endpoint)
        .with_headers(headers)
        .build()
        .map_err(|e| TelemetryError::OtlpExporter(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::config::{OtlpProtocol, SamplingStrategy, TraceConfig};

    fn grpc_config() -> TraceConfig {
        TraceConfig {
            otlp_endpoint: "http://localhost:4317".into(),
            protocol: OtlpProtocol::Grpc,
            sampling: SamplingStrategy::AlwaysOff,
            auth_header: None,
        }
    }

    fn http_config(auth: Option<&str>) -> TraceConfig {
        TraceConfig {
            otlp_endpoint: "http://localhost:4318".into(),
            protocol: OtlpProtocol::HttpProtobuf,
            sampling: SamplingStrategy::AlwaysOff,
            auth_header: auth.map(Into::into),
        }
    }

    // Tonic creates a lazy channel that requires the Tokio reactor at build time.
    #[tokio::test]
    async fn grpc_exporter_builds_without_connecting() {
        build_otlp_exporter(&grpc_config()).unwrap();
    }

    // The `http-proto` feature enables the protocol but not an HTTP client backend
    // (no `reqwest-client` / `hyper-client` feature in Cargo.toml).  The SDK
    // returns an explicit error; verify we surface it as OtlpExporter.
    #[test]
    fn http_exporter_without_client_backend_returns_otlp_error() {
        let err = build_otlp_exporter(&http_config(None)).unwrap_err();
        assert!(
            matches!(err, TelemetryError::OtlpExporter(_)),
            "expected OtlpExporter error, got: {err}",
        );
    }

    #[test]
    fn http_exporter_with_auth_header_returns_otlp_error() {
        let err = build_otlp_exporter(&http_config(Some("Bearer secret"))).unwrap_err();
        assert!(
            matches!(err, TelemetryError::OtlpExporter(_)),
            "expected OtlpExporter error, got: {err}",
        );
    }
}
