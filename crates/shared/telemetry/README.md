# `telemetry` — Unified Observability Bootstrap for core-platform

## 🎯 Overview & Service Role

`telemetry` is the single, authoritative observability crate for the core-platform workspace. It wires structured logging, OTLP distributed tracing, and Prometheus/OTLP metrics into one `init()` call, returning a lifetime-scoped guard that owns all pipeline shutdown handles.

**Every microservice binary depends on this crate** and calls `init()` before spawning gRPC or HTTP servers. This enforces a uniform observability contract across the entire platform — same log schema, same span attributes, same metric labels — with no per-service wiring boilerplate.

**Core objectives:**
- **Single call, zero drift** — one `TelemetryConfig::from_env()` + `init()` boots the full pipeline; no service-level wiring code.
- **Graceful shutdown** — the `TelemetryGuard` drop sequence flushes all in-flight spans, metrics, and log records before the process exits.
- **Hyperscale-safe defaults** — non-blocking log I/O, async batch span export, and 10 % head-sampling out of the box; none of these block the application hot path.

---

## 📐 Architecture & Concepts

### Pipeline Layout

```
Binary main()
│
├─ TelemetryConfig::from_env("service-name", version)
│
└─ telemetry::init(config)
      │
      ├─ [Layer 1] EnvFilter
      │    RUST_LOG  ──► directives (e.g. "info,my_crate=debug")
      │    LOG_FILTER ──► fallback when RUST_LOG is absent
      │
      ├─ [Layer 2] Log Layer  (tracing_appender non-blocking)
      │    tokio writer thread ◄──── bounded channel ◄──── application
      │    │
      │    ├── JSON  (default, production)
      │    └── Pretty (LOG_FORMAT=pretty, local dev)
      │
      ├─ [Layer 3] Trace Layer  (OTel bridge)
      │    SpanExporter ◄── OtlpProtocol
      │    │                ├── gRPC/tonic  → collector:4317  (default)
      │    │                └── HTTP/proto  → collector:4318
      │    BatchSpanProcessor (Tokio async, non-blocking)
      │    TracerProvider
      │    │   ├── Resource { service.name, service.version }
      │    │   └── Sampler  { AlwaysOn | AlwaysOff | TraceIdRatio(f64) }
      │    OpenTelemetryLayer ──► bridges tracing spans into OTel SDK
      │
      ├─ tracing_subscriber::Registry
      │    .with(EnvFilter) .with(log_layer) .with(trace_layer)
      │    .try_init()  ──► installs as process-global subscriber
      │
      └─ Metrics Pipeline  (independent of subscriber registry)
           ├── Prometheus  ──► SdkMeterProvider + Registry
           │                   PrometheusHandle exposed via TelemetryGuard
           │                   GET /metrics ◄── Prometheus scraper (pull)
           └── OTLP        ──► SdkMeterProvider + PeriodicReader
                               push every 60 s  ──► collector:4317

TelemetryGuard (returned to caller, must live until process exit)
  _log_guard:        WorkerGuard   — flush log writer thread on drop
  tracer_provider:   TracerProvider — shutdown() flushes buffered spans
  metrics_pipeline:  MetricsPipeline — shutdown() flushes meter provider
```

### Drop / Shutdown Sequence

When the process is about to exit and `_guard` falls out of scope:

1. `TracerProvider::shutdown()` — synchronous flush; all buffered OTLP spans are sent to the collector. Errors are printed to `stderr` and do not panic.
2. `MetricsPipeline::shutdown()` — flushes the `SdkMeterProvider`; errors are printed to `stderr`.
3. `WorkerGuard` drop — joins the background log writer thread; all buffered log records are flushed to stdout.

### Resilience Guarantees & High-Load Behaviour

| Subsystem | Backpressure / Buffer | Behaviour at Capacity |
|---|---|---|
| **Log layer** | `tracing_appender` internal bounded channel | Records are **silently dropped** if the channel fills; the application is never blocked. |
| **Trace layer** | `BatchSpanProcessor` internal queue (`opentelemetry_sdk` defaults) | Spans are **dropped** when the batch queue is full; no backpressure to the hot path. |
| **Metrics (OTLP)** | `PeriodicReader` (60 s interval) | One export attempt per period; transient collector failures are retried next period; no in-memory accumulation beyond one interval. |
| **Metrics (Prometheus)** | `prometheus::Registry` in-memory | Unbounded in-memory accumulation; scrape latency is irrelevant to the application. |
| **Init guard** | One global subscriber slot | A second `init()` call returns `TelemetryError::SubscriberInit`; the process can handle this error and continue with the first pipeline. |

---

## 🔌 Public Interfaces & API Contract

### Entry Point

```rust
// src/init.rs
pub fn init(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError>
```

Installs a `tracing_subscriber::Registry` with three layers (EnvFilter → log → trace) as the process-global subscriber, initialises the metrics pipeline independently, and returns the owning guard. **Must be called exactly once, before any `tracing::` macro.**

---

### `TelemetryConfig`

```rust
// src/config.rs
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub service_name:    String,   // → service.name OTel resource attribute
    pub service_version: String,   // → service.version OTel resource attribute
    pub log:             LogConfig,
    pub trace:           TraceConfig,
    pub metrics:         MetricsConfig,
}

impl TelemetryConfig {
    /// All fields populated from env vars; safe defaults when vars are absent.
    pub fn from_env(service_name: impl Into<String>, service_version: impl Into<String>) -> Self
}
```

---

### Sub-configs

```rust
// src/log/config.rs
pub struct LogConfig {
    pub default_filter: String,  // RUST_LOG fallback; default "info"
    pub format:         LogFormat,
    pub ansi:           bool,    // forced false in JSON mode
}

pub enum LogFormat { Json /* default */, Pretty }

// src/trace/config.rs
pub struct TraceConfig {
    pub otlp_endpoint: String,          // OTEL_EXPORTER_OTLP_ENDPOINT; default "http://localhost:4317"
    pub protocol:      OtlpProtocol,    // Grpc (default) | HttpProtobuf
    pub sampling:      SamplingStrategy,
    pub auth_header:   Option<String>,  // OTEL_EXPORTER_OTLP_HEADERS
}

pub enum OtlpProtocol { Grpc /* default, port 4317 */, HttpProtobuf /* port 4318 */ }

pub enum SamplingStrategy {
    AlwaysOn,
    AlwaysOff,
    TraceIdRatio(f64),  // must be in [0.0, 1.0]; default 0.1
}

// src/metrics/config.rs
pub struct MetricsConfig {
    pub exporter: MetricsExporterKind,
}

pub enum MetricsExporterKind {
    Prometheus,                  // default; requires `prometheus-exporter` feature
    Otlp { endpoint: String },   // push via PeriodicReader (60 s interval)
}
```

---

### `TelemetryGuard`

```rust
// src/guard.rs
pub struct TelemetryGuard { /* opaque */ }

impl TelemetryGuard {
    /// Returns the Prometheus registry handle for mounting GET /metrics.
    /// None when using OTLP metrics or when the `prometheus-exporter` feature is disabled.
    pub fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>>
}

impl Drop for TelemetryGuard { /* flushes spans → metrics → logs in order */ }
```

---

### `PrometheusHandle` & Axum route helper

```rust
// src/metrics/exporter.rs  (requires feature = "prometheus-exporter")

pub struct PrometheusHandle { /* opaque */ }

impl PrometheusHandle {
    /// Renders a Prometheus text exposition snapshot (text/plain; version=0.0.4).
    pub fn render(&self) -> String
}

/// Returns a zero-allocation Axum handler that serves the Prometheus scrape.
pub fn metrics_route(
    handle: Arc<PrometheusHandle>,
) -> impl Fn() -> Ready<([(HeaderName, HeaderValue); 1], String)> + Clone
```

---

### `TelemetryError`

```rust
// src/error.rs
#[derive(Debug, Error)]
pub enum TelemetryError {
    OtlpExporter(String),          // OTLP exporter construction failed
    Prometheus(String),            // Prometheus registry initialisation failed
    SubscriberInit(String),        // global tracing subscriber already installed
    InvalidSamplingRatio(f64),     // ratio outside [0.0, 1.0]
}
```

---

## 📦 Integration & Usage

### Cargo.toml dependency

```toml
[dependencies]
# Default features enable Prometheus scrape endpoint + Axum route helper.
telemetry = { path = "crates/shared/telemetry" }

# To disable Prometheus (pure OTLP push, removes axum + prometheus deps):
# telemetry = { path = "crates/shared/telemetry", default-features = false }
```

### Standard Bootstrap Pattern

Every service binary follows this exact sequence — no variations:

```rust
use std::sync::Arc;
use axum::{Router, routing::get};
use telemetry::{TelemetryConfig, metrics::exporter::metrics_route};

#[tokio::main]
async fn main() {
    // 1. Build config from environment variables.
    let cfg = TelemetryConfig::from_env(
        "post-command-server",
        env!("CARGO_PKG_VERSION"),
    );

    // 2. Bootstrap — must happen before any tracing:: macro.
    //    _guard must stay alive until the end of main().
    let _guard = telemetry::init(cfg).expect("telemetry init failed");

    tracing::info!("telemetry initialised, starting server");

    // 3. (Optional) Mount the Prometheus scrape endpoint.
    let prom: Arc<_> = _guard
        .prometheus_handle()
        .expect("prometheus-exporter feature is enabled by default");

    let router = Router::new()
        // Your service routes ...
        .route("/metrics", get(metrics_route(prom)));

    // 4. Start the server — _guard must remain in scope.
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, router).await.unwrap();

    // 5. _guard drops here → spans, metrics, and logs are flushed in order.
}
```

> **Critical:** Assign the guard to `_guard` (not `_`). A bare `_` binding drops immediately, flushing the pipeline before any spans are recorded.

### OTLP-only variant (no Prometheus)

```rust
use telemetry::{TelemetryConfig, metrics::config::{MetricsConfig, MetricsExporterKind}};

let cfg = TelemetryConfig {
    service_name:    "post-query-server".into(),
    service_version: env!("CARGO_PKG_VERSION").into(),
    log:             telemetry::log::config::LogConfig::from_env(),
    trace:           telemetry::trace::config::TraceConfig::from_env(),
    metrics: MetricsConfig {
        exporter: MetricsExporterKind::Otlp {
            endpoint: "http://otel-collector:4317".into(),
        },
    },
};
let _guard = telemetry::init(cfg).expect("telemetry init failed");
```

### Runtime control (live dials)

`init` returns a guard whose `control()` yields a cloneable [`TelemetryControl`].
It exposes the two operability dials that otherwise need a redeploy:

```rust
let control = _guard.control();

// Turn up a noisy module mid-incident — reloads the global `EnvFilter` live.
control.set_log_filter("info,chat=debug").unwrap();

// Drop trace volume under load (parent-based, so distributed traces stay whole).
control.set_sampling(telemetry::SamplingStrategy::TraceIdRatio(0.01)).unwrap();
```

Sampling is backed by a `DynamicSampler` (a `ShouldSample` over an `ArcSwap`),
and the log filter by a `tracing_subscriber::reload` handle — both swap
lock-free, with no restart.

In the fleet you don't call these directly: `service-runtime` registers the
control as the sink for the `[telemetry]` section of `infrastructure.toml`, so a
config push retunes log level and sampling across every pod. See
[`service-runtime`](../../platform/service-runtime/README.md) and
[`infra-config`](../infra-config/README.md).

```toml
[telemetry]
log_filter = "info,chat=debug"
sampling   = { kind = "trace_id_ratio", ratio = 0.05 }
```

---

## ⚙️ Configuration & Runtime Environment

### Environment Variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `RUST_LOG` | No | `"info"` | `tracing_subscriber` filter directives (e.g. `"info,my_crate=debug"`). Takes precedence over `LOG_FILTER`. |
| `LOG_FILTER` | No | `"info"` | Fallback filter used only when `RUST_LOG` is absent. |
| `LOG_FORMAT` | No | `json` | Log wire format. `json` (production) or `pretty` (local dev). |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | `http://localhost:4317` | OTLP **gRPC** collector address. Used for both traces and OTLP metrics. |
| `OTEL_EXPORTER_OTLP_HEADERS` | No | _(none)_ | Auth headers injected as gRPC metadata per the OTel spec. Format: `key=value,key2=value2`. Required for SaaS backends (Honeycomb, Datadog). |
| `OTEL_TRACES_SAMPLER_ARG` | No | `0.1` | Head-sampling ratio for `TraceIdRatio`. Float in `[0.0, 1.0]`. `1.0` = always sample, `0.0` = always drop. |
| `METRICS_EXPORTER` | No | `prometheus` | Metrics backend. `prometheus` (pull scrape) or `otlp` (push). |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | No | `http://localhost:4317` | OTLP collector endpoint used when `METRICS_EXPORTER=otlp`. |

### Cargo Feature Flags

| Feature | Default | Adds |
|---|---|---|
| `prometheus-exporter` | ✅ enabled | `opentelemetry-prometheus`, `prometheus` (with process metrics), `axum`; exposes `PrometheusHandle` and `metrics_route`. |

Disable with `default-features = false` to produce a binary with no Prometheus or Axum transitive dependencies.

### Runtime Requirements

- **Tokio runtime is mandatory.** `init()` must be called from within a `#[tokio::main]` context or after `Runtime::block_on`. The `BatchSpanProcessor` and OTLP `PeriodicReader` both spawn Tokio tasks via `opentelemetry_sdk::runtime::Tokio`. Calling `init()` outside a Tokio context panics inside the OTel SDK.
- **Call `init()` once.** A second call returns `TelemetryError::SubscriberInit`; the existing pipeline remains active.
- No architecture constraints (x86-64 and ARM64 supported).

---

## 📈 Telemetry, Performance & Metrics

### Automatic Process Metrics (Prometheus mode)

When the `prometheus-exporter` feature is enabled, the `prometheus` crate's `process` feature automatically registers the following gauges in the Prometheus registry at startup:

| Metric | Description |
|---|---|
| `process_cpu_seconds_total` | Total user/system CPU time consumed. |
| `process_open_fds` | Number of open file descriptors. |
| `process_max_fds` | Maximum allowed open file descriptors (`ulimit -n`). |
| `process_virtual_memory_bytes` | Virtual memory size in bytes. |
| `process_resident_memory_bytes` | Resident memory (RSS) in bytes. |
| `process_start_time_seconds` | Unix timestamp of process start. |

### Instrumenting Application Metrics

Services obtain a meter from the OTel global — no direct reference to the provider needed:

```rust
use opentelemetry::{global, KeyValue};

let meter = global::meter("post-command-server");
let requests = meter
    .u64_counter("grpc.server.requests.total")
    .with_description("Total gRPC requests handled")
    .build();

requests.add(1, &[KeyValue::new("method", "CreatePost")]);
```

### Recommended Production Alerts

| Alert | Condition | Severity |
|---|---|---|
| High log drop rate | `tracing_appender` worker thread falling behind (infer from log gaps in Loki/Grafana) | Warning |
| Span export errors | OTLP exporter returning non-OK status (visible in collector logs) | Critical |
| Process FD exhaustion | `process_open_fds / process_max_fds > 0.85` | Warning |
| High RSS growth | `process_resident_memory_bytes` growing monotonically over 10 min | Warning |

### Hot-Path Overhead

| Subsystem | Cost model |
|---|---|
| Log layer | Off-thread write via bounded channel; `~0 µs` on the hot path when sampled out by `EnvFilter`. |
| Trace layer | Span creation is `O(1)` per span; export is batched async off the critical path. |
| Metrics layer | `prometheus::Counter::inc()` is a single atomic increment; no allocation. |

---

## 🛠️ Local Development & Contribution

### Build & Check

```bash
# From workspace root
cargo build -p telemetry
cargo clippy -p telemetry -- -D warnings
cargo fmt -p telemetry --check
```

### Run Tests

```bash
cargo test -p telemetry
# With all features:
cargo test -p telemetry --all-features
# Without prometheus (verifies feature gate compiles correctly):
cargo test -p telemetry --no-default-features
```

### Local Collector (Optional)

Span export against a real OTLP collector locally:

```bash
# Start Jaeger all-in-one (gRPC on 4317, UI on 16686)
docker run --rm -p 4317:4317 -p 16686:16686 \
  jaegertracing/all-in-one:latest

export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export LOG_FORMAT=pretty
export OTEL_TRACES_SAMPLER_ARG=1.0   # sample everything locally
```

Navigate to `http://localhost:16686` to inspect traces.

### Key Files for Contributors

| File | Purpose |
|---|---|
| `src/init.rs` | Layer composition order — read this first. |
| `src/guard.rs` | Drop semantics — the shutdown sequence is here. |
| `src/trace/layer.rs` | `TracerProvider` construction and `BatchSpanProcessor` wiring. |
| `src/metrics/layer.rs` | Prometheus vs OTLP pipeline selection and `PeriodicReader` config. |
| `src/trace/exporter.rs` | gRPC vs HTTP/proto exporter dispatch and auth-header injection. |

---

## 🚨 Troubleshooting & Runbook

### 1. `TelemetryError::SubscriberInit` — "tracing subscriber already initialised"

**Cause:** `telemetry::init()` was called more than once in the same process. This is a hard constraint of the `tracing` crate's global subscriber slot.

**Mitigation:**
- Ensure `init()` is called exactly once, at the very beginning of `main()`, before any library code that might install its own subscriber (e.g., test harnesses, integration test setup).
- In integration tests, use `std::sync::OnceLock` to guard a single init:
  ```rust
  static TELEMETRY: std::sync::OnceLock<telemetry::TelemetryGuard> = std::sync::OnceLock::new();
  TELEMETRY.get_or_init(|| telemetry::init(cfg).unwrap());
  ```

---

### 2. No spans appear in the collector / "connection refused" on OTLP export

**Cause:** `OTEL_EXPORTER_OTLP_ENDPOINT` defaults to `http://localhost:4317`, which is unreachable in a Kubernetes pod unless an OTel Collector sidecar is co-located. Span export failures are **silent** — the `BatchSpanProcessor` drops spans without propagating errors to the application.

**Mitigation:**
1. Set `OTEL_EXPORTER_OTLP_ENDPOINT` to the cluster-internal collector address (e.g., `http://otel-collector.observability.svc.cluster.local:4317`).
2. Check collector logs for rejected connections or auth failures.
3. For SaaS backends requiring auth, set `OTEL_EXPORTER_OTLP_HEADERS=x-honeycomb-team=<key>` (or equivalent).
4. Temporarily set `OTEL_TRACES_SAMPLER_ARG=1.0` and `LOG_FORMAT=pretty` to confirm spans are being created before suspecting the exporter.

---

### 3. All spans sampled in production / unexpected cost spike

**Cause:** `OTEL_TRACES_SAMPLER_ARG` was explicitly set to `1.0` (always-on) or omitted in an environment that inherited a development config. Default is `0.1` (10 %).

**Mitigation:**
1. Verify the deployed env var: `kubectl exec <pod> -- env | grep OTEL_TRACES_SAMPLER_ARG`.
2. Set it to a production-appropriate value (e.g., `0.01`–`0.1`) and redeploy.
3. `SamplingStrategy::AlwaysOff` can be used to disable tracing entirely without redeploying the binary by setting `OTEL_TRACES_SAMPLER_ARG=0.0`.
