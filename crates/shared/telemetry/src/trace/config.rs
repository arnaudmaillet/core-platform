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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn otlp_protocol_default_is_grpc() {
        assert!(matches!(OtlpProtocol::default(), OtlpProtocol::Grpc));
    }

    #[test]
    fn sampling_default_is_trace_id_ratio_0_1() {
        match SamplingStrategy::default() {
            SamplingStrategy::TraceIdRatio(r) => assert!((r - 0.1).abs() < f64::EPSILON),
            other => panic!("expected TraceIdRatio, got {other:?}"),
        }
    }

    #[test]
    fn defaults_when_no_env_vars() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe {
            std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
            std::env::remove_var("OTEL_EXPORTER_OTLP_HEADERS");
            std::env::remove_var("OTEL_TRACES_SAMPLER_ARG");
        }
        let cfg = TraceConfig::from_env();
        assert_eq!(cfg.otlp_endpoint, "http://localhost:4317");
        assert!(cfg.auth_header.is_none());
        assert!(matches!(cfg.protocol, OtlpProtocol::Grpc));
    }

    #[test]
    fn custom_endpoint_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://collector:4317") };
        let cfg = TraceConfig::from_env();
        assert_eq!(cfg.otlp_endpoint, "http://collector:4317");
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT") };
    }

    #[test]
    fn sampler_arg_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("OTEL_TRACES_SAMPLER_ARG", "0.25") };
        let cfg = TraceConfig::from_env();
        match cfg.sampling {
            SamplingStrategy::TraceIdRatio(r) => assert!((r - 0.25).abs() < f64::EPSILON),
            other => panic!("expected TraceIdRatio, got {other:?}"),
        }
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("OTEL_TRACES_SAMPLER_ARG") };
    }

    #[test]
    fn invalid_sampler_arg_falls_back_to_default() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("OTEL_TRACES_SAMPLER_ARG", "not-a-number") };
        let cfg = TraceConfig::from_env();
        match cfg.sampling {
            SamplingStrategy::TraceIdRatio(r) => assert!((r - 0.1).abs() < f64::EPSILON),
            other => panic!("expected default TraceIdRatio(0.1), got {other:?}"),
        }
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("OTEL_TRACES_SAMPLER_ARG") };
    }

    #[test]
    fn auth_header_from_env() {
        let _g = ENV_LOCK.lock().unwrap();
        // SAFETY: single-threaded under the mutex lock above.
        unsafe { std::env::set_var("OTEL_EXPORTER_OTLP_HEADERS", "x-honeycomb-team=abc123") };
        let cfg = TraceConfig::from_env();
        assert_eq!(cfg.auth_header.as_deref(), Some("x-honeycomb-team=abc123"));
        // SAFETY: cleanup.
        unsafe { std::env::remove_var("OTEL_EXPORTER_OTLP_HEADERS") };
    }
}
