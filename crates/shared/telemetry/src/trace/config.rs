/// Controls the distributed-tracing layer.
#[derive(Debug, Clone)]
pub struct TraceConfig {
    /// OTLP collector gRPC endpoint — e.g. `http://jaeger:4317`.
    /// Reads `OTEL_EXPORTER_OTLP_ENDPOINT`; defaults to `http://localhost:4317`.
    pub otlp_endpoint: String,
    /// OTLP wire protocol.  Defaults to [`OtlpProtocol::Grpc`].
    pub protocol: OtlpProtocol,
    /// Span sampling strategy.  Defaults to [`SamplingStrategy::TraceIdRatio(0.1)`].
    pub sampling: SamplingStrategy,
    /// Optional bearer token forwarded as the `Authorization` header to the
    /// OTLP collector (Honeycomb, Datadog, etc.).
    /// Reads `OTEL_EXPORTER_OTLP_HEADERS`.
    pub auth_header: Option<String>,
}

/// OTLP transport protocol.
#[derive(Debug, Clone, Default)]
pub enum OtlpProtocol {
    /// gRPC (port 4317) — preferred for high-throughput intra-cluster traffic.
    #[default]
    Grpc,
    /// HTTP/protobuf (port 4318) — broadest SaaS vendor compatibility.
    HttpProtobuf,
}

/// Span sampling strategy wired into the OTel SDK.
#[derive(Debug, Clone)]
pub enum SamplingStrategy {
    /// Forward every span — development / low-traffic services only.
    AlwaysOn,
    /// Drop every span — disables tracing without redeploying.
    AlwaysOff,
    /// Probabilistic head-based sampling; value must be in `[0.0, 1.0]`.
    TraceIdRatio(f64),
}

impl Default for SamplingStrategy {
    fn default() -> Self {
        Self::TraceIdRatio(0.1)
    }
}

impl TraceConfig {
    /// Reads `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS`,
    /// and `OTEL_TRACES_SAMPLER_ARG` following the OTel environment-variable
    /// specification.
    pub fn from_env() -> Self {
        let otlp_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".into());

        let auth_header = std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok();

        let sampling = std::env::var("OTEL_TRACES_SAMPLER_ARG")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .map(SamplingStrategy::TraceIdRatio)
            .unwrap_or_default();

        Self {
            otlp_endpoint,
            protocol: OtlpProtocol::Grpc,
            sampling,
            auth_header,
        }
    }
}
