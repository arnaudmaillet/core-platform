# `telemetry` — Domain & Functional Contract

> The observability bootstrap: logs + OTLP traces + metrics in one `init()`. It answers *"how does every binary get uniform observability with one call and live retuning?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | One-call observability bootstrap: structured logging + OTLP distributed tracing + Prometheus/OTLP metrics, with lock-free live dials |
> | **Layer** | `platform` — the single observability bootstrap every binary calls (via `service-runtime`) |
> | **Subdomain class** | **Supporting** — the observability substrate; high operational leverage (one schema, live retuning) |
> | **Primary abstraction(s)** | `init` + `TelemetryGuard` + `TelemetryControl` (`telemetry`) |
> | **Footprint** | IO/stateful — installs the process-global subscriber, spawns OTLP/metrics tasks; requires a Tokio runtime |
> | **Failure posture** | **failure-tolerant** — buffers drop at capacity rather than backpressure the app; export failures are silent + retried |
> | **Depends on** | `tracing(-subscriber)`, `opentelemetry(_sdk)`, `arc-swap`, `prometheus`/`axum` (optional) |
> | **Consumed by** | `service-runtime` (calls `init` in `serve`); storage crates emit into the installed subscriber |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `telemetry` is the fleet's authority for **the observability pipeline**: it answers
**"how does a binary stand up logs, traces, and metrics with one schema and one call — and retune log filter
and sampling live during an incident without a restart?"**

**The hard problem.** Three independent pipelines (logging, tracing, metrics) must initialise in the right
order, flush in the right order on shutdown, never block the hot path, and expose lock-free live dials for
incident response. `telemetry` wires all three behind one `init()` returning a lifetime-scoped guard whose drop
flushes spans → metrics → logs in order, and a `TelemetryControl` that swaps the log filter and sampler over
`ArcSwap`.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Define application metrics → services obtain a meter from the OTel global.
- ❌ Poll health or own readiness → that is `health` + `service-runtime`.
- ❌ Decide when/where it's called → in the fleet, `service-runtime` calls `init`, not services directly.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Init | The single bootstrap call; install the process-global subscriber + pipelines | `init`, `TelemetryConfig` |
| Guard | The lifetime-scoped handle whose drop flushes everything in order | `TelemetryGuard` |
| Control | The cloneable live dials (log filter + sampling) | `TelemetryControl` |
| Sampling strategy | Head-sampling policy, parent-based to keep traces whole | `SamplingStrategy::{AlwaysOn, AlwaysOff, TraceIdRatio}` |
| Metrics exporter | Prometheus pull or OTLP push | `MetricsExporterKind`, `PrometheusHandle` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `init(config)` | entrypoint | Call **once**, inside a Tokio runtime, before any `tracing::` macro |
| `TelemetryGuard` | RAII guard | Bind to a **named** var; drop = ordered flush (spans → metrics → logs); errors print, never panic |
| `TelemetryControl` | live dials | `set_log_filter`/`set_sampling` swap lock-free, no restart; parent-based sampling keeps traces whole |
| `TelemetryConfig::from_env` | config | Reads `RUST_LOG`/`OTEL_*`/`LOG_FORMAT`/`METRICS_EXPORTER` |
| `PrometheusHandle` / `metrics_route` | feature surface | `prometheus-exporter` (default on); `GET /metrics` text |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- Pipeline construction, ordered shutdown, and the live-retuning dials. One log schema, one span-attribute
  convention, one metric-label convention.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| Application metric definitions | each service (via OTel global meter) | The crate provides the pipeline, not the business metrics |
| The `[telemetry]` config section parsing | `infra-config` | It exposes a `TelemetrySink`; `service-runtime` bridges it (the two crates must not depend on each other) |
| Health/readiness | `health` + `service-runtime` | Separate concern |

**The "do-not-depend-on" list:** never `infra-config` (the bridge lives in `service-runtime`), never a service
crate. `prometheus`/`axum` are optional (feature-gated).

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `init` is called exactly once (the `tracing` global slot is single) | global subscriber | `TelemetryError::SubscriberInit` |
| I2 | `init` runs inside a Tokio runtime, before any `tracing::` macro | OTel SDK | panic (tasks spawn) / dropped early events |
| I3 | The guard is bound to a named var and outlives the process | RAII | early flush; logs/spans vanish at exit |
| I4 | The hot path never blocks on telemetry (drop-at-capacity, silent export failures) | non-blocking writers + batch export | data loss preferred over backpressure |
| I5 | Sampling is parent-based so distributed traces stay whole | `DynamicSampler` | broken traces when volume is dropped |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Init.** Build a `Registry` with `EnvFilter` (`RUST_LOG`|`LOG_FILTER`) + a non-blocking JSON/Pretty log layer
(`tracing_appender`) + an OTLP trace layer (gRPC:4317 | HTTP:4318 → `BatchSpanProcessor` → `TracerProvider`
{Resource, Sampler}) → `try_init()` installs the process-global subscriber. A metrics pipeline (Prometheus pull
or OTLP push every 60s) is built independently. Returns `TelemetryGuard { _log_guard, tracer_provider,
metrics_pipeline }`.

**Live dials.** `TelemetryControl` holds a `tracing_subscriber::reload` handle (log filter) and a
`DynamicSampler` (`ShouldSample` over `ArcSwap`); both swap lock-free with no restart. `service-runtime`
registers this control as the sink for the `[telemetry]` config section, so a ConfigMap push retunes the fleet.

**Shutdown.** Guard drop flushes in order: `TracerProvider::shutdown()` (spans) → `MetricsPipeline::shutdown()`
→ `WorkerGuard` (join the log thread). Errors print to stderr, never panic. A second `init()` returns
`SubscriberInit` rather than clobbering the first.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `service-runtime` | downstream | Published Contract | `init` + `TelemetryControl` | observability boot + live dials |
| `transport` | downstream | Conformist | global propagator + pinned OTel versions | trace propagation (wire-compatible context) |
| storage/service crates | downstream | Conformist | emit into the installed subscriber | their logs/spans appearing at all |
| `infra-config` | indirect | Separated Interface | `TelemetrySink` (bridged in `service-runtime`) | live config-driven retuning |

> **Stability seam:** `init`/`TelemetryGuard`/`TelemetryControl` are public API; the pinned OTel versions are a
> wire-compatibility contract with `transport`.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| process gauges | Prometheus metrics | `prometheus-exporter` on | `process_cpu_seconds_total`, `process_open_fds`, RSS, … |
| exported spans / metrics | OTLP | batched off the hot path | the collector (Jaeger/Honeycomb/Datadog) |
| every service's logs/spans/metrics | relayed | whenever a service instruments | this is the substrate they ride on |

Side effects: installs the global subscriber, spawns the batch-span + OTLP-reader Tokio tasks, optionally serves
`/metrics`.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| One `init()` for all three pipelines; guard drop = ordered flush | [`README §Architecture`](../README.md) | Accepted |
| Lock-free live dials (`TelemetryControl`) for log filter + parent-based sampling | [`README §Architecture`](../README.md) | Accepted |
| Failure-tolerant by design (drop-at-capacity, silent export retries) | [`README §Architecture`](../README.md) | Accepted |
| Bridge to `infra-config` lives in `service-runtime` (avoid a circular dep) | [`service-runtime README`](../../service-runtime/README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Supporting — the observability substrate; leverage is one schema fleet-wide + live
  retuning.
- **Stability:** stable contract.
- **Volatility:** low-medium — exporter/back-end options evolve; the `init`/guard/control shape is settled.
- **Deferred capabilities:** none structural; new exporters or sampling strategies are additive enums.
