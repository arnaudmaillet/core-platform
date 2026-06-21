use telemetry::{
    TelemetryConfig, TelemetryError,
    log::config::{LogConfig, LogFormat},
    metrics::config::{MetricsConfig, MetricsExporterKind},
    trace::config::{OtlpProtocol, SamplingStrategy, TraceConfig},
};

/// A minimal config that does not emit logs/spans and does not require a
/// running OTLP collector.  Safe to use in a unit-test environment.
fn silent_config(name: &str) -> TelemetryConfig {
    TelemetryConfig {
        service_name: name.into(),
        service_version: "0.0.0-test".into(),
        log: LogConfig {
            default_filter: "off".into(),
            format: LogFormat::Json,
            ansi: false,
        },
        trace: TraceConfig {
            otlp_endpoint: "http://127.0.0.1:4317".into(),
            protocol: OtlpProtocol::Grpc,
            // AlwaysOff — no spans emitted, so the batch processor never
            // tries to send anything to the (absent) collector.
            sampling: SamplingStrategy::AlwaysOff,
            auth_header: None,
        },
        metrics: MetricsConfig {
            exporter: MetricsExporterKind::Prometheus,
        },
    }
}

/// This test is the only place in the entire test suite that calls
/// `telemetry::init()`.  Integration-test binaries each get their own
/// process, so the global tracing subscriber starts fresh here.
#[tokio::test]
async fn init_returns_guard_and_second_call_errors() {
    // ── first call: must succeed ──────────────────────────────────────────────
    let guard = telemetry::init(silent_config("integration-test"))
        .expect("first init must succeed");

    // Guard exposes a live Prometheus handle (default feature enabled).
    let handle = guard
        .prometheus_handle()
        .expect("prometheus handle must be Some with default features");

    // render() must not panic and must return valid UTF-8.
    let text = handle.render();
    assert!(text.is_ascii(), "prometheus output is not ASCII: {text:?}");

    // ── second call: must fail with SubscriberInit ────────────────────────────
    let result = telemetry::init(silent_config("integration-test-2"));
    assert!(result.is_err(), "second init must fail because global subscriber is already installed");
    // .err().unwrap() avoids the T: Debug bound that .unwrap_err() requires.
    assert!(
        matches!(result.err().unwrap(), TelemetryError::SubscriberInit(_)),
        "expected SubscriberInit error variant",
    );

    // Leak the guard instead of dropping it: TracerProvider::shutdown() blocks
    // trying to flush to the absent gRPC collector.  The Tokio runtime will
    // clean up background tasks when the test process exits.
    std::mem::forget(guard);
}
