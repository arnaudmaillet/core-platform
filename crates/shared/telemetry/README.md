# `telemetry` — Interface Contract

## 🎯 Service Role

Single-call bootstrap for the core-platform observability pipeline: it wires a non-blocking structured-log layer (JSON/Pretty to stdout via `tracing_appender`), an OTLP/gRPC distributed-tracing layer (batch-flushed `TracerProvider` bridged through `tracing-opentelemetry`), and a metrics pipeline (Prometheus pull or OTLP push via `SdkMeterProvider`), then installs a single `tracing_subscriber::Registry` as the global subscriber and returns a `TelemetryGuard` that owns all pipeline shutdown handles.

---

## 🔌 Public Interfaces (Traits & API)

### `init` — the only function a binary calls

```rust
// src/init.rs
pub fn init(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError>
```

Composes a `tracing_subscriber::Registry` with three layers in this order:

| Priority | Layer | Source file |
|---|---|---|
| 1st | `EnvFilter` | `RUST_LOG` or `config.log.default_filter` |
| 2nd | `fmt::Layer` (non-blocking JSON or Pretty) | `src/log/layer.rs` |
| 3rd | `OpenTelemetryLayer` (OTLP/gRPC or HTTP/proto) | `src/trace/layer.rs` |

The `SdkMeterProvider` (Prometheus or OTLP) is initialised independently and stored inside `TelemetryGuard`.

---

### `TelemetryConfig` — root configuration aggregator

```rust
// src/config.rs
pub struct TelemetryConfig {
    pub service_name:    String,   // embedded in every log record, span, and metric label
    pub service_version: String,
    pub log:             LogConfig,
    pub trace:           TraceConfig,
    pub metrics:         MetricsConfig,
}

impl TelemetryConfig {
    /// Builds from env vars; all vars optional with safe defaults.
    pub fn from_env(service_name: impl Into<String>, service_version: impl Into<String>) -> Self
}
```

---

### `TelemetryGuard` — shutdown anchor

```rust
// src/guard.rs
pub struct TelemetryGuard { /* opaque */ }

impl TelemetryGuard {
    /// `Arc<PrometheusHandle>` for mounting GET /metrics; None when OTLP metrics are used
    /// or the `prometheus-exporter` feature is disabled.
    pub fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>>
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        // 1. TracerProvider::shutdown() — flushes all in-flight OTLP spans
        // 2. SdkMeterProvider::shutdown() — flushes in-flight metrics
        // 3. WorkerGuard drop — flushes the non-blocking log writer thread
    }
}
```

---

### Sub-config structs

```rust
// src/log/config.rs
pub struct LogConfig {
    pub default_filter: String,   // RUST_LOG fallback; default "info"
    pub format:         LogFormat, // Json (default) | Pretty
    pub ansi:           bool,
}

// src/trace/config.rs
pub struct TraceConfig {
    pub otlp_endpoint: String,          // default "http://localhost:4317"
    pub protocol:      OtlpProtocol,    // Grpc (default) | HttpProtobuf
    pub sampling:      SamplingStrategy, // TraceIdRatio(0.1) default
    pub auth_header:   Option<String>,
}

// src/metrics/config.rs
pub struct MetricsConfig {
    pub exporter: MetricsExporterKind, // Prometheus (default) | Otlp { endpoint }
}
```

---

### `PrometheusHandle` — scrape endpoint helper

```rust
// src/metrics/exporter.rs  (requires feature = "prometheus-exporter")
impl PrometheusHandle {
    pub fn render(&self) -> String
    // Returns Prometheus text/plain; version=0.0.4
}

pub fn metrics_route(
    handle: Arc<PrometheusHandle>,
) -> impl Fn() -> Ready<([ContentTypeHeader; 1], String)> + Clone
// Mount as: router.route("/metrics", axum::routing::get(metrics_route(handle)))
```

---

## 📦 Entry Points & Consumption

### Add to a service

```toml
# service Cargo.toml
telemetry = { path = "crates/shared/telemetry" }
# default features enable Prometheus scrape endpoint + axum route helper
# to opt out: telemetry = { ..., default-features = false }
```

### Bootstrap pattern — every binary follows this exact sequence

```rust
#[tokio::main]
async fn main() {
    // 1. Build config (reads env vars)
    let cfg = telemetry::TelemetryConfig::from_env(
        "post-command-server",
        env!("CARGO_PKG_VERSION"),
    );

    // 2. Bootstrap — must happen before any tracing:: macro
    let _guard = telemetry::init(cfg).expect("telemetry init failed");

    // 3. Optional — mount Prometheus scrape route
    let prom = _guard.prometheus_handle().expect("prometheus-exporter feature enabled");
    let router = axum::Router::new()
        .route("/metrics", axum::routing::get(
            telemetry::metrics::exporter::metrics_route(prom),
        ));

    // 4. Run the service — _guard must stay alive
    // server.serve(...).await.unwrap();
}
```

### Required runtime environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `RUST_LOG` | no | `"info"` | `tracing_subscriber` filter directive |
| `LOG_FORMAT` | no | `json` | `json` or `pretty` |
| `LOG_FILTER` | no | `"info"` | Fallback filter when `RUST_LOG` is absent |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | no | `http://localhost:4317` | Jaeger / Honeycomb / Datadog gRPC collector |
| `OTEL_EXPORTER_OTLP_HEADERS` | no | — | `key=value,key2=value2` — injected as gRPC metadata per OTel spec |
| `OTEL_TRACES_SAMPLER_ARG` | no | `0.1` | Head-sampling ratio `[0.0, 1.0]` |
| `METRICS_EXPORTER` | no | `prometheus` | `prometheus` or `otlp` |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | no | `http://localhost:4317` | Used when `METRICS_EXPORTER=otlp` |

### Tokio runtime requirement

`init()` must be called from inside a `#[tokio::main]` context (or after `Runtime::block_on`).
The `BatchSpanProcessor` and the OTLP metrics `PeriodicReader` both spawn Tokio tasks via
`opentelemetry_sdk::runtime::Tokio`.

### Cargo features

| Feature | Default | Effect |
|---|---|---|
| `prometheus-exporter` | ✅ yes | Enables `opentelemetry-prometheus`, `prometheus`, and `axum` deps; exposes `PrometheusHandle` and `metrics_route` |

---

## 📝 Key Files for Context

| File | What to read |
|---|---|
| `src/init.rs` | Layer composition order and what arguments `TelemetryGuard::new` receives — understand this before any other file |
| `src/guard.rs` | `TelemetryGuard` drop semantics — critical for not silently losing spans or log records at shutdown |
| `src/config.rs` + sub-configs in `src/{log,trace,metrics}/config.rs` | Complete env-var contract and all tuneable knobs — read these before wiring any service |
