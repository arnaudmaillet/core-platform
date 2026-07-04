# `transport` — Domain & Functional Contract

> The shared communication layer: gRPC + Kafka behind one API, answering *"how does any service talk to any other with end-to-end traces and at-least-once delivery, for free?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Inter-service communication over two paradigms (sync gRPC + async Kafka) with automatic W3C trace propagation and a mandatory consumer runtime |
> | **Layer** | `platform` — the single wire layer every service uses |
> | **Subdomain class** | **Generic** — commodity transport; leverage is uniform tracing + the `run_consumer` standard |
> | **Primary abstraction(s)** | `GrpcClientBuilder`/`GrpcServerBuilder`, `KafkaProducerHandle`/`KafkaConsumerHandle`, `run_consumer` (`transport::{grpc, kafka}`) |
> | **Footprint** | IO/stateful — owns sockets, the Kafka client, and the consumer loop |
> | **Failure posture** | **fail-fast egress** (`CircuitOpen`/`Timeout`) + **at-least-once Kafka** (commit only after a terminal outcome) |
> | **Depends on** | `tonic`/`tower`, `rdkafka`, `resilience`, `traffic`, `telemetry`, `error`, `opentelemetry` |
> | **Consumed by** | every service (gRPC clients/servers, Kafka producers/consumers) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `transport` is the fleet's authority for **the wire**: it answers
**"how does a service call another (gRPC) or publish/consume an event (Kafka) such that traces propagate
end-to-end and delivery semantics are uniform?"** — with zero boilerplate per service.

**The hard problem.** Two transports, one trace story. Both must auto-propagate W3C TraceContext
(`traceparent`/`tracestate`), wire in resilience (egress) and traffic (ingress) layers, and give Kafka a
*single* correct at-least-once consumer runtime so retry/DLQ/offset behaviour is identical fleet-wide rather
than re-invented (and subtly broken) per service.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Own domain types, schemas, or handler logic → it owns only the wire + trace plumbing.
- ❌ Retry at the gRPC channel level → HTTP/2 bodies are streams (buffering cost); retry goes at the app layer.
- ❌ Define the resilience/traffic *mechanism* → those are `resilience`/`traffic`; this crate wires them.
- ❌ Initialise telemetry → `telemetry::init()` is a hard prerequisite (registers the propagator).

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Resilient channel | A gRPC channel wrapped with trace + circuit-breaker + timeout | `ResilientChannel`, `GrpcClientBuilder::build_resilient` |
| Traced server | A gRPC server with inbound-trace + ingress-traffic layers pre-installed | `GrpcServerBuilder`, `TracedGrpcServer` |
| Event envelope | The typed Kafka publish carrier | `EventEnvelope<T>`, `PublishablePayload` |
| Consumed message | A decoded inbound Kafka message (decode error = `payload: Err`, not a stream abort) | `ConsumedMessage<T>`, `ConsumablePayload` |
| Consumer runtime | The mandatory per-message state machine (retry/DLQ/commit) | `run_consumer`, `ProcessOutcome`, `ClassifyError` |
| Propagation | Inject/extract trace context across both transports | `inject_context`, `extract_context`, `set_parent` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `TransportError` | error envelope | Flattens gRPC/Kafka/Codec + `CircuitOpen`/`Timeout`/`MaxRetriesExhausted` |
| `ResilientChannel` | type alias | `BoxCloneService<…, TransportError>`; cheaply `Clone`; reads CB/timeout from a `ResilienceProfile` `ArcSwap` |
| `KafkaProducerHandle` | handle | `Arc`-backed, `Clone`; `publish` injects trace context into headers |
| `KafkaConsumerHandle` | handle | `stream` (decode error ≠ abort) + `commit` (offset+1, manual commit default) |
| `run_consumer` | runtime | Owns the retry/DLQ/commit state machine — **mandatory** for every consumer |
| `ProcessOutcome` | enum | `Done`/`Retry`/`Reject` drive the runner's terminal-vs-redeliver decision |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The wire (gRPC + Kafka clients/servers), the trace plumbing, the resilience/traffic layer *wiring*, and the
  consumer runtime. Delivery semantics (the `run_consumer` standard) are owned here.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| The resilience mechanism (CB/retry/timeout) | `resilience` | This crate *wires* it; it doesn't define it |
| The traffic limiter | `traffic` | Same — `TrafficLayer` is wired but inert until a registry is supplied |
| Telemetry pipeline + global propagator | `telemetry` | `transport` requires `init()` first; it doesn't own the pipeline |
| Domain event schemas / handler logic | service crates | Transport carries opaque payloads + trace context |

**The "do-not-depend-on" list:** never a service/domain crate. OTel versions are pinned to `telemetry`'s for
wire-compatible context.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `telemetry::init()` runs before any transport call (else inject/extract are no-ops) | runtime prerequisite | disconnected spans / missing `traceparent` |
| I2 | A Kafka offset commits only after a **terminal** outcome (success or successful dead-letter) | `run_consumer` | a poison message is evicted without loss |
| I3 | A broker/stream error or DLQ-publish failure returns `Err` **without** committing | `run_consumer` | resume from last committed offset, no loss |
| I4 | A decode failure dead-letters immediately (does not abort the stream) | `stream` + `run_consumer` | poison isolated to the DLQ |
| I5 | No `RetryLayer` at the channel level | client stack composition | (retry belongs at the app layer) |
| I6 | Idempotency is the consumer's responsibility (at-least-once ⇒ real redelivery) | contract convention | duplicate side-effects |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**gRPC client stack.** `TimeoutLayer → CircuitBreakerLayer → OutboundTraceLayer → tonic Channel`
(`ResilientChannel`). The outbound layer injects `traceparent`/`tracestate` into the HTTP/2 headers; CB and
timeout read hot-reloadable values from the originating `ResilienceProfile`'s `ArcSwap`.

**gRPC server stack.** `InboundTraceLayer` (outer — traces even throttled requests) `→ TrafficLayer` (ingress
limit, inert until `service-runtime` supplies a `TrafficRegistry`; shadow mode charges cells without
rejecting) `→ handler`.

**Kafka consumer (`run_consumer`).** Per message: decode (`payload: Err` ⇒ dead-letter + commit); else run
`process` → `ProcessOutcome`: `Done` ⇒ commit; `Retry` ⇒ in-place backoff+jitter up to `max_attempts`, then
dead-letter + commit; `Reject` ⇒ dead-letter + commit. A broker error or **DLQ publish failure** ⇒ return
`Err` without committing, so the caller rebuilds and resumes from the last committed offset. DLQ records carry
`x-dlq-origin-*` + the trace context.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `resilience` | upstream | Conformist | `*Layer`s + `ResilienceProfile` | egress resilience wiring |
| `traffic` | upstream | Conformist | `check` / `TrafficDecision` → `RESOURCE_EXHAUSTED` | ingress limiting |
| `telemetry` | upstream | Conformist | global propagator + pinned OTel versions | trace propagation |
| `error` | upstream | Conformist | `grpc_severity`, error mapping | gRPC error severity |
| every service | downstream | Published Contract | client/server builders + `run_consumer` | all inter-service comms |
| `traffic-redis` | downstream-of-us (injected) | Separated Interface | `with_traffic_backend(Arc<dyn QuotaBackend>)` | distributed rate limiting |

> **Stability seam:** `run_consumer` + its delivery table is a **mandatory fleet standard**; `TransportError`,
> the builders, and the Kafka handles are public API.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| `grpc.server` span | `tracing`/OTel | each inbound RPC (`rpc.system=grpc`, `rpc.method`) | trace backends |
| injected `traceparent`/`tracestate` | wire header | each gRPC client call + Kafka publish | the receiver's extract |
| DLQ record | Kafka side-effect | a terminal `Retry`-exhausted/`Reject`/decode failure | DLQ consumers / ops |
| `infra_traffic_throttled_total{status}` | metric (via traffic wiring) | a `Throttle` decision (shadow or enforce) | rate-limit dashboards |

Side effects: opens sockets, publishes/consumes Kafka, writes DLQ records, commits offsets.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Both transports auto-propagate W3C TraceContext | [`README §Architecture`](../README.md) | Accepted |
| No `RetryLayer` at the transport level (HTTP/2 body buffering) — retry at the app layer | [`README §Architecture`](../README.md) | Accepted |
| `run_consumer` is the mandatory consumer runtime; commit only after a terminal outcome | [`README §Consumer runtime standard`](../README.md) | Accepted |
| Traffic wired-but-inert until configured; shadow mode before enforce | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — commodity transport; the differentiation is uniform tracing + a single,
  correct at-least-once consumer runtime.
- **Stability:** stable contract — `run_consumer` is a fleet standard; the builders/handles are settled.
- **Volatility:** low-medium — growth is in observability (Prometheus meter instruments are a TODO) and
  config knobs, not the shape.
- **Deferred capabilities:** Prometheus meter instruments for transport-level metrics; richer TLS/mTLS
  rollout.
