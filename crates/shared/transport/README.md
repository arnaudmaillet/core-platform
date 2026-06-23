# `transport` — Platform-Wide Communication Layer

## 🎯 Overview & Service Role

`transport` is the **single, shared infrastructure crate** that every microservice in the platform depends on for inter-process communication. It provides a uniform, opinionated API over two complementary messaging paradigms:

- **Synchronous RPC** — gRPC via [Tonic](https://github.com/hyperium/tonic) + [Tower](https://github.com/tower-rs/tower), with optional TLS/mTLS, circuit-breaking, and timeout protection.
- **Asynchronous event-driven messaging** — Kafka via [rdkafka](https://github.com/fede1024/rust-rdkafka), with typed envelopes and at-least-once delivery semantics.
- **Payload serialization** — shared JSON and Protobuf codecs used across both transports.

**The defining property:** both transports automatically propagate distributed trace context ([W3C TraceContext](https://www.w3.org/TR/trace-context/) — `traceparent` / `tracestate`) across every RPC call and every Kafka message boundary, producing end-to-end traces with zero boilerplate in consuming services.

This crate owns **no business logic**. It is a pure infrastructure library.

---

## 📐 Architecture & Concepts

### Module layout

```
transport/
├── error.rs                        TransportError (unified) + CodecError
├── propagation/
│   ├── carrier.rs                  MetadataCarrier supertrait + inject_context / extract_context
│   ├── grpc.rs                     GrpcHeaderInjector / GrpcHeaderExtractor  (http::HeaderMap)
│   └── kafka.rs                    KafkaHeaderInjector / KafkaHeaderExtractor (OwnedHeaders / BorrowedHeaders)
├── codec/
│   ├── json.rs                     encode<T: Serialize> / decode<T: DeserializeOwned>
│   └── protobuf.rs                 encode<M: prost::Message> / decode<M: prost::Message + Default>
├── grpc/
│   ├── error.rs                    GrpcTransportError + grpc_severity()
│   ├── layer/
│   │   ├── outbound.rs             OutboundTraceLayer — zero-cost, injects trace on every outbound call
│   │   ├── inbound.rs              InboundTraceLayer  — BoxFuture, extracts trace from every inbound call
│   │   └── traffic.rs              TrafficLayer       — ingress rate limiting → RESOURCE_EXHAUSTED
│   ├── client/
│   │   ├── config.rs               GrpcClientConfig · GrpcTlsConfig · GrpcResilienceConfig
│   │   └── builder.rs              GrpcClientBuilder → Channel | OutboundTraceService<Channel> | ResilientChannel
│   └── server/
│       ├── config.rs               GrpcServerConfig · GrpcServerTlsConfig
│       └── builder.rs              GrpcServerBuilder → TracedGrpcServer
└── kafka/
    ├── error.rs                    KafkaTransportError
    ├── envelope.rs                 EventEnvelope<T> · PublishablePayload · ConsumablePayload
    ├── config/
    │   ├── client.rs               KafkaClientConfig  (shared broker + SASL settings)
    │   ├── producer.rs             ProducerConfig     (acks, compression, linger_ms, in-flight)
    │   └── consumer.rs             ConsumerConfig     (group_id, offset reset, heartbeat, session)
    ├── producer/
    │   ├── builder.rs              KafkaProducerBuilder → KafkaProducerHandle
    │   └── handle.rs               KafkaProducerHandle.publish<T>() / publish_raw()
    └── consumer/
        ├── builder.rs              KafkaConsumerBuilder → KafkaConsumerHandle
        ├── handle.rs               KafkaConsumerHandle.stream<T>() / commit() · ConsumedMessage<T>
        └── runner.rs               run_consumer · RetryPolicy · ProcessOutcome · ClassifyError
```

### Data flow — distributed trace propagation

```
┌──────────────────────────────────────────────────────────────────────┐
│  Service A (caller)                                                  │
│                                                                      │
│   tracing::info_span!("handle_request")                             │
│       │                                                              │
│       ├─ [gRPC path]                                                 │
│       │   OutboundTraceLayer                                         │
│       │     inject_context → GrpcHeaderInjector → http::HeaderMap   │
│       │     ────────────── HTTP/2 ──────────────────────────────►   │
│       │                                                              │
│       └─ [Kafka path]                                                │
│           KafkaProducerHandle::publish                               │
│             inject_context → KafkaHeaderInjector → OwnedHeaders     │
│             ──────────────── Kafka record ──────────────────────►   │
└──────────────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────────────┐
│  Service B (receiver)                                                │
│                                                                      │
│   ├─ [gRPC path]                                                     │
│   │   InboundTraceLayer                                              │
│   │     extract_context ← GrpcHeaderExtractor ← http::HeaderMap     │
│   │     span.set_parent(remote_cx) → child of Service A's span      │
│   │                                                                  │
│   └─ [Kafka path]                                                    │
│       KafkaConsumerHandle::stream                                    │
│         extract_context ← KafkaHeaderExtractor ← BorrowedHeaders    │
│         Span::current().set_parent(remote_cx)                        │
└──────────────────────────────────────────────────────────────────────┘
```

### gRPC client Tower stack (outermost → innermost)

This is exactly what `build_resilient` / `build_from_registry` compose (then type-erase to `ResilientChannel`):

```
TimeoutLayer              (from resilience crate)
  └─ map_err(TransportError::from_resilience)
      └─ CircuitBreakerLayer     (from resilience crate)
          └─ map_err(TransportError::from_resilience_connect)
              └─ OutboundTraceLayer   ← injects traceparent / tracestate
                  └─ tonic::transport::Channel
```

### gRPC server Tower stack

```
tonic::Server
  └─ InboundTraceLayer            (outer: every request, incl. throttled, is traced)
      └─ TrafficLayer             (inner: ingress rate limiting; pass-through if unconfigured)
          ├─ resolves the method's [traffic] profile from the registry
          ├─ extracts a key by scope (per_method, or per_caller via edge-mesh identity)
          ├─ charges one cell against the governor limiter
          ├─ enforce=true  + over quota → short-circuit RESOURCE_EXHAUSTED (+ retry-after-ms)
          ├─ enforce=false (shadow)     → log would-throttle, admit
          └─ otherwise → handler future inside the span
```

### Activating ingress rate limiting (server)

`TrafficLayer` is always present in the server type but is a no-op until a
`TrafficRegistry` is supplied. The serving binary wires it once at boot:

```rust,ignore
// 1. Load + resolve the whole infrastructure.toml, validating every section.
let infra = Arc::new(InfraRegistry::from_config(load_from_path("/etc/infra/infrastructure.toml".as_ref())?)?);

// 2. Hot-reload on ConfigMap change (keep the guard alive). One watcher drives every
//    section, so traffic quotas / enforce flags flip with no redeploy.
let _watcher = spawn_watcher("/etc/infra/infrastructure.toml".into(), Arc::clone(&infra));

// 3. Install the layer (only when a [traffic] section is configured).
let mut builder = GrpcServerBuilder::new(GrpcServerConfig::default());
if let Some(traffic) = infra.traffic() {
    builder = builder.with_traffic(Arc::clone(&traffic));

    // 4. Bound memory for unbounded keyspaces (per_caller): sweep idle keys on an interval.
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        loop { tick.tick().await; traffic.prune_all(); }
    });
}
let server = builder.build()?.add_service(/* … */);
```

**Safe rollout (pilot = post-command writes).** Ship the tight profile with
`enforce = false` (shadow): the limiter charges cells without rejecting anything.
Watch the **`infra_traffic_throttled_total`** counter (labels: `profile`, `route`,
`status`) — specifically the `status="shadow"` series, which is exactly what *would*
be rejected. When the rate looks right, edit `enforce = true` in the ConfigMap: it
hot-reloads to enforcement (`status` flips to `enforced`) with no redeploy, and
reverts just as fast. Route cardinality is bounded — unbound methods collapse to a
single `route="<unbound>"` label.

### Resilience Guarantees & High-Load Behavior

| Concern | gRPC | Kafka |
|---|---|---|
| **Circuit breaking** | `CircuitBreakerLayer` (from `resilience` crate) wraps the channel; open state → `TransportError::CircuitOpen` | N/A — broker failures surface as `KafkaTransportError::Producer/Consumer` |
| **Timeout** | `TimeoutLayer` per-call deadline; exceeded → `TransportError::Timeout(Duration)` | rdkafka internal `Timeout::Never` on produce; caller controls consumer poll cadence |
| **Retry** | **Intentionally absent at this layer.** HTTP/2 bodies are streams — replaying them requires buffering the full payload, which is cost-prohibitive. Apply `RetryLayer` at the **application layer** around the generated tonic client call, before serialization. | At-least-once via `acks = "all"` + manual commit after success. `run_consumer` adds bounded in-place retry (exponential backoff + jitter) and dead-letters poison / retry-exhausted records to `{topic}.dlq` — see [Consumer runtime standard](#consumer-runtime-standard) |
| **Backpressure** | Tower's `poll_ready` propagates upstream naturally | rdkafka's internal queue; `linger_ms` + `max_in_flight` tune batching vs. ordering |
| **Memory** | No request body buffering at this layer | `EventEnvelope<T>` is deserialized per-message; consumer stream is lazy (pull-based) |
| **TLS / mTLS** | `GrpcClientConfig::with_tls()` + `GrpcServerConfig::with_tls()` (optional client CA for mTLS) | SASL_SSL via `KafkaClientConfig` security settings |

---

## 🔌 Public Interfaces & API Contract

### `TransportError` — unified error type

```rust
pub enum TransportError {
    Grpc(GrpcTransportError),       // tonic::transport::Error or tonic::Status
    Kafka(KafkaTransportError),     // rdkafka::error::KafkaError
    Codec(CodecError),              // serde_json / prost encode/decode failures
    CircuitOpen,                    // CircuitBreakerLayer rejected the call
    Timeout(Duration),              // TimeoutLayer deadline exceeded
    MaxRetriesExhausted(u32),       // RetryLayer (application-layer callers)
}
```

Ergonomic `From` impls: `tonic::transport::Error`, `tonic::Status`, `GrpcTransportError`, `KafkaTransportError`, `CodecError` all convert via `?`.

Two flattening helpers for resilience layer integration:

```rust
// Flatten ResilienceError<tonic::transport::Error> produced during channel connect
TransportError::from_resilience_connect(e: ResilienceError<tonic::transport::Error>) -> Self

// Flatten ResilienceError<TransportError> from any wrapped resilience layer
TransportError::from_resilience(e: ResilienceError<TransportError>) -> Self
```

### `GrpcTransportError` — severity mapping

```rust
pub enum GrpcTransportError {
    Connect(tonic::transport::Error),
    Status { code: tonic::Code, message: String },
    InvalidMetadata(String),
    Tls(String),
}

// Maps tonic::Code → error::Severity for structured alerting
pub fn grpc_severity(code: tonic::Code) -> error::Severity {
    // Critical  → Internal, DataLoss, Unknown
    // High      → Unavailable, DeadlineExceeded, ResourceExhausted
    // Medium    → PermissionDenied, Unauthenticated
    // Low       → everything else
}
```

### `GrpcClientBuilder`

```rust
pub struct GrpcClientBuilder { /* private */ }

impl GrpcClientBuilder {
    pub fn new(config: GrpcClientConfig) -> Self;

    // Raw channel — no middleware. Share across multiple tonic clients or compose manually.
    pub async fn connect(self) -> Result<tonic::transport::Channel, TransportError>;

    // Channel wrapped in OutboundTraceLayer. Concrete, cloneable, usable directly with
    // generated tonic clients. Add resilience layers on top via ServiceBuilder.
    pub async fn build_traced(self)
        -> Result<OutboundTraceService<tonic::transport::Channel>, TransportError>;

    // Full stack: trace + circuit breaker + timeout from a ResilienceProfile,
    // flattened to TransportError and type-erased. Hot-reloadable via the profile's
    // ArcSwap handles. Drops straight into a generated tonic client.
    pub async fn build_resilient(self, profile: &ResilienceProfile)
        -> Result<ResilientChannel, TransportError>;

    // Same, but resolves the profile from the registry using GrpcClientConfig::dependency
    // (its [resilience.bindings] key). The registry-driven entry point.
    pub async fn build_from_registry(self, registry: &ResilienceRegistry)
        -> Result<ResilientChannel, TransportError>;
}

// Type-erased, cloneable, hot-reloadable client stack:
pub type ResilientChannel = BoxCloneService<
    http::Request<tonic::body::Body>,
    http::Response<tonic::body::Body>,
    TransportError,
>;
```

`ResilientChannel` is `Clone` (tonic clones the service per RPC) and reads its circuit-breaker / timeout config from the originating `ResilienceProfile`'s shared `ArcSwap` handles — so a control-plane hot-swap (via `infra-config`) reconfigures the live channel with no rebuild. `RetryLayer` remains absent at this layer (HTTP/2 body replay); apply retry at the application layer.

### `GrpcClientConfig`

```rust
pub struct GrpcClientConfig {
    pub endpoint: String,                       // e.g. "https://svc:50051"
    pub dependency: String,                     // [resilience.bindings] key; defaults to endpoint
    pub tls: Option<GrpcTlsConfig>,             // None = plaintext (service-mesh mTLS at sidecar)
    pub connect_timeout: Duration,              // default: 5s
    pub resilience: Option<GrpcResilienceConfig>, // static fallback; superseded by build_from_registry
}
// Set the binding key: GrpcClientConfig::new(uri).with_dependency("post-command")
```

### `GrpcServerBuilder` / `TracedGrpcServer`

```rust
pub type TracedGrpcServer = Server<Stack<TrafficLayer, Stack<InboundTraceLayer, Identity>>>;

pub struct GrpcServerBuilder { /* private */ }

impl GrpcServerBuilder {
    pub fn new(config: GrpcServerConfig) -> Self;
    // Enable ingress rate limiting (optional; without it the traffic layer is a no-op).
    pub fn with_traffic(self, registry: Arc<infra_config::TrafficRegistry>) -> Self;
    // Returns a server with InboundTraceLayer + TrafficLayer pre-installed.
    pub fn build(self) -> Result<TracedGrpcServer, TransportError>;
}
```

### `EventEnvelope<T>`

```rust
pub struct EventEnvelope<T> {
    pub topic: String,
    pub key: String,                            // partition key — use stable domain ID for ordering
    pub payload: T,
    pub headers: HashMap<String, String>,       // user headers; trace headers are transport-internal
    pub timestamp_ms: Option<i64>,              // None = broker-assigned creation time
}

// Marker traits (blanket-implemented)
pub trait PublishablePayload: Serialize + Send + Sync + 'static {}
pub trait ConsumablePayload: DeserializeOwned + Send + Sync + 'static {}
```

### `KafkaProducerHandle`

```rust
#[derive(Clone)]                                // Arc-backed FutureProducer — cheap to clone
pub struct KafkaProducerHandle { /* private */ }

impl KafkaProducerHandle {
    // Serializes payload to JSON, injects trace context, publishes to broker.
    pub async fn publish<T: PublishablePayload>(
        &self,
        envelope: EventEnvelope<T>,
    ) -> Result<(), TransportError>;

    // Bypasses JSON serialization. Trace context is still injected automatically.
    pub async fn publish_raw(
        &self,
        topic: &str,
        key: &str,
        payload: &[u8],
        user_headers: HashMap<String, String>,
    ) -> Result<(), TransportError>;
}
```

### `KafkaConsumerHandle`

```rust
pub struct ConsumedMessage<T> {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub key: String,
    pub headers: HashMap<String, String>,
    pub timestamp_ms: Option<i64>,
    pub raw_payload: Vec<u8>,                 // original bytes — retained for dead-lettering
    pub payload: Result<T, TransportError>,   // Err = poison/decode failure (offset still known)
}

impl KafkaConsumerHandle {
    // Lazy async stream. Each item: trace context extracted + parent span set + payload decoded.
    // A decode failure is surfaced as `payload = Err(..)` (it does NOT abort the stream), so the
    // worker can dead-letter the poison record and commit past it instead of stalling the partition.
    pub fn stream<T: ConsumablePayload>(
        &self,
    ) -> impl Stream<Item = Result<ConsumedMessage<T>, TransportError>> + '_;

    // Commit the offset past `msg` (offset + 1). Required when enable_auto_commit = false (default).
    pub fn commit<T>(&self, msg: &ConsumedMessage<T>) -> Result<(), TransportError>;
}
```

### Consumer runtime standard

> **Every Kafka consumer in the platform runs on `run_consumer`.** Do not hand-roll the
> stream/commit loop. The runner owns the per-message state machine, so retry, dead-lettering,
> and offset management are defined once and behave identically across all services.

```rust
pub enum ProcessOutcome { Done, Retry(String), Reject(String) }

pub trait ClassifyError { fn is_retryable(&self) -> bool; }
impl ProcessOutcome {
    pub fn from_result<E: ClassifyError + Display>(r: Result<(), E>) -> Self;
}

pub struct RetryPolicy { pub max_attempts: u32, pub base_backoff: Duration, pub max_backoff: Duration }
// Default: 5 attempts · 100 ms base · 30 s cap · exponential backoff with equal jitter.

pub async fn run_consumer<T: ConsumablePayload, F>(
    handle:   &KafkaConsumerHandle,
    producer: &KafkaProducerHandle,   // emits to the dead-letter topic
    policy:   &RetryPolicy,
    process:  F,                       // for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output=ProcessOutcome> + Send + 'a>>
) -> Result<(), TransportError>;
```

**Delivery semantics (the standard):**

| Outcome | Runner action |
|---|---|
| `Done` (success or intentional skip) | commit the offset |
| `Retry` (transient fault) | in-place exponential backoff + jitter, up to `max_attempts`, **then** dead-letter + commit |
| `Reject` (permanent / poison data) | dead-letter immediately + commit |
| decode failure (`payload = Err`) | dead-letter immediately + commit |
| broker/stream error, or **dead-letter publish failure** | return `Err` **without committing** → caller rebuilds the consumer and resumes from the last committed offset (no message loss) |

Committing only after a terminal outcome (success *or* a successful dead-letter publish) is what
evacuates a poison message from its partition **without ever losing it** — the record is durably
parked on `{origin_topic}.dlq` before its offset advances. Dead-letter records carry diagnostic
headers: `x-dlq-origin-topic`, `x-dlq-partition`, `x-dlq-offset`, `x-dlq-reason`
(`decode` / `reject` / `retry-exhausted`), `x-dlq-error`, `x-dlq-attempts`, `x-dlq-failed-at-ms`,
plus the propagated trace context.

**Worker authoring rules:**

1. **Configure for at-least-once:** `ConsumerConfig` with `enable_auto_commit = false` (the default).
2. **Classify your errors:** `impl ClassifyError for YourError` — transient storage/cache faults
   are retryable; validation / bad-data / invariant violations are not. Where a domain error
   already implements `error::AppError`, delegate: `<Self as AppError>::is_retryable(self)`.
3. **Fold intentional skips into `Ok`** (e.g. block-gated, self-targeted, cache-miss) so they
   commit cleanly instead of flooding the DLQ.
4. **Own the handle via `Arc<Self>`** so the per-message closure captures an owned handle (the
   returned future then borrows only the event, which the runner's bound requires):

```rust
pub async fn run(self) {
    let producer = build_dlq_producer(&self.kafka_config)?; // shared per-service helper
    let worker = Arc::new(self);
    loop {
        match worker.clone().run_once(&producer).await {
            Ok(())  => tracing::warn!("consumer exited cleanly — restarting"),
            Err(e)  => { tracing::error!(%e, "consumer error — restarting after 5 s"); sleep(5s).await; }
        }
    }
}

async fn run_once(self: Arc<Self>, producer: &KafkaProducerHandle) -> Result<(), String> {
    let handle = KafkaConsumerBuilder::new(config).subscribe(TOPIC).build()?;
    run_consumer::<MyEvent, _>(&handle, producer, &RetryPolicy::default(), move |event| {
        let worker = Arc::clone(&self);
        Box::pin(async move { ProcessOutcome::from_result(worker.process(event).await) })
    }).await.map_err(|e| e.to_string())
}
```

> **`process` is handed only the decoded event, not the topic.** A consumer whose logic depends on
> *which* topic a record came from (e.g. a `created` / `deleted` pair) should run one
> single-topic `run_consumer` loop per topic, sharing the same `group_id` to preserve committed-offset
> continuity.
>
> **Idempotency is the consumer's responsibility.** At-least-once means real redelivery; handlers
> must be idempotent (deterministic keys, claim-gated counters, etc.) so retries do not double-apply.

#### Closure lifetime papercut (HRTB inference)

`run_consumer`'s `process` bound is **higher-ranked**: `for<'a> Fn(&'a T) -> Pin<Box<dyn Future<… + 'a>>>`.
When you pass the closure **inline** as the argument (as in the example above), the compiler reads that
bound as the expected type and infers the closure correctly — this is the path to prefer.

The trap appears only when you bind the closure to a `let` first **and its future does not borrow the
event** (e.g. it ignores the payload, or clones everything it needs). The compiler then infers a single
concrete lifetime and rejects it against the `for<'a>` bound:

```text
error[E0308]: mismatched types
   = note: expected `Pin<Box<dyn Future<…> + Send>>`
              found `Pin<Box<dyn Future<…> + Send + 'a>>`
   = note: one type is more general than the other
```

Route the `let`-bound closure through an identity coercion whose signature *is* the higher-ranked bound,
which forces the expected-typed inference:

```rust
fn classify<T, F>(process: F) -> F
where
    F: for<'a> Fn(&'a T) -> Pin<Box<dyn Future<Output = ProcessOutcome> + Send + 'a>> + Send + 'static,
{
    process
}

let process = classify::<MyEvent, _>(move |event| {
    let worker = Arc::clone(&self);
    Box::pin(async move { ProcessOutcome::from_result(worker.process(event).await) })
});
run_consumer::<MyEvent, _>(&handle, producer, &RetryPolicy::default(), process).await
```

(The runtime's own integration suite under `tests/` uses exactly this helper; see
`tests/consumer_runtime.rs`.)

### `propagation` module

```rust
// Inject the current tracing span's OTel context into any carrier
pub fn inject_context<C: Injector>(carrier: &mut C);

// Extract a remote OTel Context from any carrier (returns root ctx if no headers present)
pub fn extract_context<C: Extractor>(carrier: &C) -> opentelemetry::Context;

// Wire a remote context as the parent of a local tracing span
pub fn set_parent(span: &tracing::Span, cx: opentelemetry::Context);
```

| Carrier | Transport | Direction | Backing type |
|---|---|---|---|
| `GrpcHeaderInjector<'_>` | gRPC | Outbound | `&mut http::HeaderMap` |
| `GrpcHeaderExtractor<'_>` | gRPC | Inbound | `&http::HeaderMap` |
| `KafkaHeaderInjector` | Kafka | Outbound | `OwnedHeaders` (builder via `mem::replace`) |
| `KafkaHeaderExtractor<'_>` | Kafka | Inbound | `&BorrowedHeaders` |

### `codec` module

```rust
// JSON
pub fn json_encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError>;
pub fn json_decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError>;

// Protobuf (prost)
pub fn proto_encode<M: prost::Message>(msg: &M) -> Result<Bytes, CodecError>;
pub fn proto_decode<M: prost::Message + Default>(bytes: &[u8]) -> Result<M, CodecError>;
```

Both return `CodecError`, which maps to `TransportError::Codec` via `From`.

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# service/Cargo.toml
[dependencies]
transport = { path = "crates/shared/transport" }
```

### Standard bootstrap pattern

`telemetry::init()` **must be called first** — it registers the global W3C TraceContext propagator that every `inject_context` / `extract_context` call reads.

```rust
use transport::{
    grpc::{
        client::{builder::GrpcClientBuilder, config::GrpcClientConfig},
        server::{builder::GrpcServerBuilder, config::GrpcServerConfig},
    },
    kafka::{
        config::{client::KafkaClientConfig, consumer::ConsumerConfig, producer::ProducerConfig},
        consumer::builder::KafkaConsumerBuilder,
        producer::builder::KafkaProducerBuilder,
        envelope::EventEnvelope,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Telemetry FIRST — registers the global OTel propagator
    let _guard = telemetry::init(
        telemetry::TelemetryConfig::from_env("my-service", env!("CARGO_PKG_VERSION"))
    )?;

    // 2a. gRPC server — InboundTraceLayer pre-installed
    let server = GrpcServerBuilder::new(GrpcServerConfig::default())
        .build()?
        .add_service(MyServiceServer::new(MyServiceImpl))
        .serve("0.0.0.0:50051".parse()?);

    // 2b. gRPC client — resilient channel resolved from the profile registry.
    // The registry is loaded from infrastructure.toml and hot-reloaded by a watcher;
    // see the `infra-config` crate.
    use std::sync::Arc;
    use infra_config::{InfrastructureConfig, ResilienceRegistry, spawn_watcher};

    let registry = Arc::new(ResilienceRegistry::from_config(
        InfrastructureConfig::from_toml(&std::fs::read_to_string("infrastructure.toml")?)?,
    )?);
    let _watcher = spawn_watcher("infrastructure.toml".into(), Arc::clone(&registry))?;

    let channel = GrpcClientBuilder::new(
        GrpcClientConfig::new("https://dependency-svc:50051").with_dependency("post-command")
    )
    .build_from_registry(&registry)   // resolves "post-command" → bound profile; hot-reloads
    .await?;

    let mut client = DependencyServiceClient::new(channel);

    // 2c. Kafka producer
    let producer = KafkaProducerBuilder::new(
        ProducerConfig::new(KafkaClientConfig::from_env())
    )
    .build()?;

    producer.publish(
        EventEnvelope::new("domain.events", "entity-id-123", MyEvent { /* ... */ })
            .with_header("x-source-service", "my-service")
    ).await?;

    // 2d. Kafka consumer
    let consumer = KafkaConsumerBuilder::new(
        ConsumerConfig::new(KafkaClientConfig::from_env(), "my-service-consumer")
    )
    .subscribe("domain.events")
    .build()?;

    let mut stream = consumer.stream::<MyEvent>();
    while let Some(result) = stream.next().await {
        let envelope = result?;
        // Span is already linked to the producer's trace — no manual wiring needed
        process(envelope.payload).await?;
        // Commit only after successful processing (enable_auto_commit = false by default)
        // consumer.commit(&raw_msg)?;
    }

    server.await?;
    Ok(())
}
```

---

## ⚙️ Configuration & Runtime Environment

### Kafka environment variables (`KafkaClientConfig::from_env()`)

| Variable | Required | Default | Description |
|---|---|---|---|
| `KAFKA_BROKERS` | No | `localhost:9092` | Comma-separated broker addresses, e.g. `kafka-0:9092,kafka-1:9092` |
| `KAFKA_SECURITY_PROTOCOL` | No | `PLAINTEXT` | `PLAINTEXT` for in-cluster, `SASL_SSL` for cloud-managed (MSK, Confluent) |
| `KAFKA_SASL_MECHANISM` | No | — | `PLAIN`, `SCRAM-SHA-256`. Required when protocol is `SASL_*` |
| `KAFKA_SASL_USERNAME` | No | — | SASL username |
| `KAFKA_SASL_PASSWORD` | No | — | SASL password |
| `KAFKA_DEBUG` | No | — | rdkafka internal debug log scope: `all`, `consumer`, `producer`, `topic` |

> OpenTelemetry exporter configuration (`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_SERVICE_NAME`, etc.) is managed by the `telemetry` crate. See `crates/shared/telemetry/README.md`.

### gRPC — programmatic configuration (no env vars)

gRPC client and server are configured via builder types, not environment variables. Key defaults:

| Field | Default | Notes |
|---|---|---|
| `GrpcClientConfig::connect_timeout` | `5s` | TCP + TLS handshake deadline |
| `GrpcResilienceConfig::timeout` | `10s` | Per-call deadline |
| `GrpcResilienceConfig::circuit_breaker` | `CircuitBreakerConfig::default()` | See `resilience` crate |
| `GrpcServerConfig::addr` | `0.0.0.0:50051` | Bind address |
| `GrpcServerConfig::tls` | `None` | Plaintext; suitable for service-mesh mTLS at sidecar |
| `GrpcServerConfig::enable_reflection` | `false` | Enable for dev/staging (`grpcurl`, Postman) |

### Kafka producer defaults (`ProducerConfig::default()`)

| Field | Default | Description |
|---|---|---|
| `acks` | `"all"` | Leader + all in-sync replicas — maximum durability |
| `compression` | `"snappy"` | Good throughput/CPU trade-off |
| `linger_ms` | `5` | Batch window in ms before flush |
| `max_in_flight` | `5` | In-flight produce requests per broker |

### Kafka consumer defaults (`ConsumerConfig`)

| Field | Default | Description |
|---|---|---|
| `auto_offset_reset` | `Latest` | Skip backlog on first start; use `Earliest` for event replay |
| `enable_auto_commit` | `false` | Manual commit required — gives at-least-once control |
| `heartbeat_interval_ms` | `3000` | Must be < broker `session.timeout.ms` |
| `session_timeout_ms` | `10000` | Broker considers consumer dead after this |

---

## 📈 Telemetry, Performance & Metrics

### Execution prerequisites

- **Async runtime:** Tokio (`tokio = { features = ["full"] }`). Both `tonic` and `rdkafka`'s `FutureProducer` / `StreamConsumer` require a Tokio runtime to be active.
- **`telemetry::init()` first:** The global W3C TraceContext propagator is registered there. Calling `inject_context` or `extract_context` before `init()` is a no-op — trace headers will not be propagated.
- **OTel versions pinned:** `opentelemetry = "0.27"`, `tracing-opentelemetry = "0.28"`. These are pinned to the same versions as the `telemetry` crate to guarantee wire-compatible context propagation.

### Automatic OTel spans

| Span name | Transport | Side | Key attributes |
|---|---|---|---|
| `grpc.server` | gRPC | Server | `rpc.system = "grpc"`, `rpc.method = "/pkg.Service/Method"` |
| _(injected into existing span)_ | gRPC | Client | `traceparent`, `tracestate` injected into HTTP headers |
| _(set_parent on current span)_ | Kafka | Consumer | Remote `traceparent` extracted and wired as parent |

### Recommended production alerts

| Alert | Condition | Severity |
|---|---|---|
| gRPC circuit open | `TransportError::CircuitOpen` rate > threshold | **Critical** |
| gRPC timeout rate | `TransportError::Timeout` rate > 1% of calls | **High** |
| Kafka producer error | `KafkaTransportError::Producer` non-zero | **High** |
| Kafka consumer lag | Consumer group offset lag > SLA threshold | **High** |
| Codec error | `TransportError::Codec` non-zero | **Medium** — indicates schema mismatch |

> Metric instrumentation (Prometheus counters/histograms) is a `<!-- TODO: planned — add OTel meter instruments to InboundTraceService and KafkaProducerHandle -->`.

---

## 🛠️ Local Development & Contribution

### Build & lint

```bash
# Build the crate
cargo build -p transport

# Run the hermetic unit tests (no Docker)
cargo test -p transport

# Lint
cargo clippy -p transport -- -D warnings

# Format
cargo fmt -p transport
```

### Live-broker integration suite (`run_consumer`)

The consumer-runtime fault-tolerance tests live under `tests/` and are gated behind the
`integration-kafka` feature so the default `cargo test` stays Docker-free. They are **self-contained**:
an ephemeral single-node broker (`apache/kafka-native`, KRaft) is booted via `testcontainers` — no
`docker compose`, no `localhost:9092`. A running Docker daemon is the only prerequisite.

```bash
# Boots one container, runs Scenarios A–K against it (~16 s)
cargo test -p transport --features integration-kafka
```

`tests/harness/mod.rs` owns the broker plumbing (one shared container per binary, UUIDv7-namespaced
topics/groups per test, explicit topic pre-creation, an `await_until` poll primitive — never `sleep`);
`tests/consumer_runtime.rs` holds the scenarios (happy path, poison/decode, reject, transient retry,
retry-exhaustion, backoff+jitter envelope, ordering/no-stall, DLQ header completeness, key affinity,
and the at-least-once "failed dead-letter ⇒ no commit + redelivery" proof).

### Key architectural invariants

1. **`telemetry::init()` before any transport call** — without the global propagator, `inject_context` and `extract_context` are silent no-ops.
2. **`KafkaProducerHandle` is `Clone`** — `FutureProducer` is `Arc`-backed internally via rdkafka. Share freely across Tokio tasks.
3. **`OutboundTraceService<Channel>` is `Clone`** — both `Channel` and `OutboundTraceService` are cheap to clone. Use directly with generated tonic clients.
4. **`InboundTraceLayer` changes the future type** (`BoxFuture`) because `Instrument` wraps the inner future in a new type. `OutboundTraceLayer` is zero-cost — `type Future = S::Future` unchanged.
5. **Never use `tonic::body::BoxBody` in public type signatures** — it is private in tonic 0.14.x.
6. **`KafkaHeaderInjector` uses `mem::replace`** — `OwnedHeaders::insert` is a consuming builder; ownership is temporarily moved out via `std::mem::replace` to satisfy the `&mut self` `Injector` contract.
7. **No `RetryLayer` at the transport level** — HTTP/2 bodies are streams; replaying them requires buffering the full payload. Apply `RetryLayer` at the application layer (around the tonic client call, not the channel).
8. **Consumer offset commitment is the caller's responsibility** — `enable_auto_commit = false` by default. Call `consumer.commit(&raw_msg)` or `Consumer::commit_message` after successful processing.

---

## 🚨 Troubleshooting & Runbook

### 1. Trace headers not propagated — spans appear disconnected in Jaeger/Tempo

**Symptom:** `traceparent` is absent from gRPC headers or Kafka records; all spans appear as root spans.

**Root cause:** `telemetry::init()` was not called before the first transport operation. The global OTel propagator is registered by `telemetry::init()` — without it, `inject_context` and `extract_context` are no-ops.

**Fix:**
```rust
// Ensure this runs BEFORE any GrpcClientBuilder / KafkaProducerBuilder call
let _guard = telemetry::init(TelemetryConfig::from_env("svc", env!("CARGO_PKG_VERSION")))?;
```
Keep `_guard` alive for the duration of `main`. Dropping it shuts down the OTel pipeline.

---

### 2. `TransportError::CircuitOpen` — requests rejected immediately

**Symptom:** `TransportError::CircuitOpen` returned without ever reaching the remote service.

**Root cause:** The `CircuitBreakerLayer` has tripped due to repeated failures on the upstream dependency. The circuit remains open for the configured recovery window.

**Immediate mitigation:**
1. Check the health of the upstream gRPC service (`kubectl get pods`, gRPC health probe).
2. Review recent errors in structured logs (`TransportError::Grpc(Status { code: Unavailable, ... })`).
3. The circuit will close automatically once the upstream recovers and the half-open probe succeeds.
4. For transient spikes: tune `CircuitBreakerConfig` thresholds or add `RetryLayer` at the application layer.

---

### 3. Kafka consumer not receiving messages / stuck at startup

**Symptom:** `KafkaConsumerHandle::stream()` yields nothing; `Kafka consumer subscribed` log appears but no messages follow.

**Root cause (most common):** `auto_offset_reset = Latest` (default) and the consumer group has no committed offset. The consumer starts at the tip of the partition and waits for new messages — existing messages in the topic are skipped.

**Fix:** For event replay or first-time bootstrap, set `auto_offset_reset = Earliest`:
```rust
let cfg = ConsumerConfig {
    auto_offset_reset: AutoOffsetReset::Earliest,
    ..ConsumerConfig::new(KafkaClientConfig::from_env(), "my-group")
};
```

**Secondary root cause:** two instances sharing the same `group_id` with the same number of partitions. One consumer may hold all partitions, leaving the other idle. Verify partition-to-consumer assignment via `kafka-consumer-groups.sh --describe`.
