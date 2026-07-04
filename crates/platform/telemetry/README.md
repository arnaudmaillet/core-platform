# `telemetry` — Unified observability bootstrap: logs + OTLP traces + metrics in one `init()`

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `platform` — the single observability bootstrap every binary calls |
> | **Package** | `telemetry` (dir: `crates/platform/telemetry`) |
> | **Consumed by** | `service-runtime` (calls `init` in `serve`); storage crates emit into the installed subscriber |
> | **Depends on** | `tracing(-subscriber)`, `opentelemetry(_sdk)`, `arc-swap`, `prometheus`/`axum` (optional) |
> | **Stability** | stable contract |
> | **Feature flags** | `prometheus-exporter` (default **on**) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`telemetry` is the single, authoritative observability crate: it wires structured logging, OTLP
distributed tracing, and Prometheus/OTLP metrics into one `init()` call, returning a lifetime-scoped
`TelemetryGuard` that owns all pipeline shutdown handles. Every binary calls it before serving — one
log schema, one set of span attributes, one metric-label convention, with no per-service wiring.

**Architectural boundary** — it owns pipeline construction + shutdown + the live retuning dials. It
does **not** define application metrics (services obtain a meter from the OTel global) and does not
poll health. In the fleet, services don't call `init` directly — `service-runtime` does.

**Core objectives:** single call / zero drift; graceful shutdown (the guard's drop flushes spans →
metrics → logs in order); hyperscale-safe defaults (non-blocking log I/O, async batch span export,
10% head-sampling — none block the hot path).

---

## 📐 Architecture & key decisions

```
init(config):
  Registry .with(EnvFilter: RUST_LOG|LOG_FILTER)
           .with(LogLayer: tracing_appender non-blocking → JSON | Pretty)
           .with(TraceLayer: OTLP gRPC:4317 | HTTP:4318 → BatchSpanProcessor → TracerProvider{Resource, Sampler})
           .try_init()                                   ← process-global subscriber
  + Metrics pipeline (independent): Prometheus (pull /metrics) | OTLP (push every 60s)
  → TelemetryGuard { _log_guard, tracer_provider, metrics_pipeline }   (must live until process exit)
```

- **Guard drop = ordered flush** — on drop: `TracerProvider::shutdown()` (flush spans) →
  `MetricsPipeline::shutdown()` → `WorkerGuard` (join log thread). Errors print to stderr, never panic.
- **Live dials via `TelemetryControl`** — log filter (a `tracing_subscriber::reload` handle) and
  sampling (a `DynamicSampler` = `ShouldSample` over `ArcSwap`) both swap **lock-free, no restart**.
  Sampling is parent-based, so distributed traces stay whole when you drop volume mid-incident.
  `service-runtime` registers this control as the sink for the `[telemetry]` config section.
- **Failure-tolerant by design** — span/log buffers **drop** at capacity rather than backpressure the
  app; OTLP export failures are silent (retried next batch/period). A second `init()` returns
  `SubscriberInit` instead of clobbering the first pipeline.

---

## 🔌 Public API & contract

```rust
pub fn init(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError>;   // call ONCE, before any tracing:: macro

pub struct TelemetryConfig { pub service_name: String, pub service_version: String, pub log: LogConfig, pub trace: TraceConfig, pub metrics: MetricsConfig }
impl TelemetryConfig { pub fn from_env(service_name: impl Into<String>, service_version: impl Into<String>) -> Self; }

pub enum LogFormat { Json, Pretty }
pub enum OtlpProtocol { Grpc, HttpProtobuf }
pub enum SamplingStrategy { AlwaysOn, AlwaysOff, TraceIdRatio(f64) }   // ratio ∈ [0,1], default 0.1
pub enum MetricsExporterKind { Prometheus, Otlp { endpoint: String } }

pub struct TelemetryGuard;
impl TelemetryGuard {
    pub fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>>;  // None for OTLP / feature off
    pub fn control(&self) -> TelemetryControl;                         // cloneable live dials
}
impl Drop for TelemetryGuard { /* spans → metrics → logs */ }

impl TelemetryControl { pub fn set_log_filter(&self, &str) -> Result<(),_>; pub fn set_sampling(&self, SamplingStrategy) -> Result<(),_>; }

// feature = "prometheus-exporter":
impl PrometheusHandle { pub fn render(&self) -> String; }             // text/plain; version=0.0.4
pub fn metrics_route(handle: Arc<PrometheusHandle>) -> impl Fn() -> /* Axum handler */ + Clone;

pub enum TelemetryError { OtlpExporter(String), Prometheus(String), SubscriberInit(String), InvalidSamplingRatio(f64) }
```

> **Contract notes:** call `init` exactly once, from within a Tokio context (the batch processor + OTLP
> reader spawn Tokio tasks — calling outside a runtime panics in the OTel SDK), before any `tracing::`
> macro. Bind the guard to a **named** variable (`let _guard = …`) — a bare `_` drops it immediately and
> flushes before anything is recorded.

---

## 📦 Integration

```toml
[dependencies]
telemetry = { workspace = true }                          # Prometheus + Axum route helper by default
# telemetry = { workspace = true, default-features = false }  # pure OTLP push, drops axum+prometheus
```

```rust
let _guard = telemetry::init(TelemetryConfig::from_env("post-command-server", env!("CARGO_PKG_VERSION")))
    .expect("telemetry init failed");                     // BEFORE serving; keep _guard alive to flush on exit
let prom = _guard.prometheus_handle().unwrap();
let router = Router::new().route("/metrics", get(telemetry::metrics::exporter::metrics_route(prom)));
```

Live retuning (fleet uses `service-runtime` + `[telemetry]` config; direct API shown):
`_guard.control().set_log_filter("info,chat=debug")`, `.set_sampling(SamplingStrategy::TraceIdRatio(0.01))`.

---

## ⚙️ Configuration & feature flags

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | `tracing_subscriber` directives; precedence over `LOG_FILTER` |
| `LOG_FILTER` | `info` | Fallback filter when `RUST_LOG` absent |
| `LOG_FORMAT` | `json` | `json` (prod) or `pretty` (dev) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC collector (traces + OTLP metrics) |
| `OTEL_EXPORTER_OTLP_HEADERS` | — | Auth headers `k=v,k2=v2` (Honeycomb/Datadog) |
| `OTEL_TRACES_SAMPLER_ARG` | `0.1` | Head-sampling ratio `[0,1]` |
| `METRICS_EXPORTER` | `prometheus` | `prometheus` (pull) or `otlp` (push) |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | `http://localhost:4317` | OTLP endpoint when `METRICS_EXPORTER=otlp` |

**Feature flags:** `prometheus-exporter` (default on) — adds `opentelemetry-prometheus`, `prometheus`
(with process metrics), `axum`; exposes `PrometheusHandle` + `metrics_route`. `default-features = false`
⇒ no Prometheus/Axum deps. **Tokio runtime is mandatory.**

---

## 🔭 Observability

Prometheus mode auto-registers process gauges: `process_cpu_seconds_total`, `process_open_fds`,
`process_max_fds`, `process_virtual_memory_bytes`, `process_resident_memory_bytes`,
`process_start_time_seconds`. Instrument app metrics via the OTel global:
`global::meter("svc").u64_counter("grpc.server.requests.total").build()`.

Suggested alerts: OTLP export errors (collector logs) ⇒ critical; `process_open_fds/max_fds > 0.85` ⇒
warn; monotonic RSS over 10m ⇒ warn; log-gap (worker behind) ⇒ warn. Hot-path overhead is ~0 when
filtered out; span creation O(1), export batched off-path; counter inc is one atomic.

---

## 🧪 Testing

```bash
cargo test   -p telemetry
cargo test   -p telemetry --all-features
cargo test   -p telemetry --no-default-features      # verify the feature gate compiles
cargo clippy -p telemetry --all-targets
# local collector: docker run --rm -p4317:4317 -p16686:16686 jaegertracing/all-in-one + OTEL_TRACES_SAMPLER_ARG=1.0
```

Key files for contributors: `src/init.rs` (layer order — read first), `src/guard.rs` (drop sequence),
`src/trace/layer.rs`, `src/metrics/layer.rs`, `src/trace/exporter.rs`.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `TelemetryError::SubscriberInit` — "subscriber already initialised".**
`init()` was called twice (the `tracing` global slot is single). Call it once at the top of `main()`
before any library installs its own subscriber. In tests, guard with a `OnceLock<TelemetryGuard>`.

**2. No spans in the collector / "connection refused" on export.**
The default endpoint `http://localhost:4317` is unreachable in a pod without a collector sidecar, and
export failures are **silent** (spans dropped). Point `OTEL_EXPORTER_OTLP_ENDPOINT` at the cluster
collector; set `OTEL_TRACES_SAMPLER_ARG=1.0` + `LOG_FORMAT=pretty` to confirm spans are created before
blaming the exporter; set `OTEL_EXPORTER_OTLP_HEADERS` for SaaS auth.

**3. All spans sampled in prod / cost spike.**
`OTEL_TRACES_SAMPLER_ARG=1.0` leaked from a dev config (default is `0.1`). Verify the deployed env, set
`0.01`–`0.1`; `0.0` disables tracing without a binary change.

**4. The pipeline flushed nothing / logs vanished at exit.**
The guard was bound to `_` (drops immediately). Use `let _guard = telemetry::init(...)?` and keep it in
scope until the end of `main()`.
