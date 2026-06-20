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
│   │   └── inbound.rs              InboundTraceLayer  — BoxFuture, extracts trace from every inbound call
│   ├── client/
│   │   ├── config.rs               GrpcClientConfig · GrpcTlsConfig · GrpcResilienceConfig
│   │   └── builder.rs              GrpcClientBuilder → Channel | OutboundTraceService<Channel>
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
        └── handle.rs               KafkaConsumerHandle.stream<T>() / commit()
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
  └─ InboundTraceLayer
      ├─ extracts traceparent / tracestate from request headers
      ├─ reconstructs remote OpenTelemetry Context
      ├─ opens a `grpc.server` span (rpc.system, rpc.method attributes)
      ├─ sets remote context as parent span
      └─ instruments the entire handler future inside that span
```

### Resilience Guarantees & High-Load Behavior

| Concern | gRPC | Kafka |
|---|---|---|
| **Circuit breaking** | `CircuitBreakerLayer` (from `resilience` crate) wraps the channel; open state → `TransportError::CircuitOpen` | N/A — broker failures surface as `KafkaTransportError::Producer/Consumer` |
| **Timeout** | `TimeoutLayer` per-call deadline; exceeded → `TransportError::Timeout(Duration)` | rdkafka internal `Timeout::Never` on produce; caller controls consumer poll cadence |
| **Retry** | **Intentionally absent at this layer.** HTTP/2 bodies are streams — replaying them requires buffering the full payload, which is cost-prohibitive. Apply `RetryLayer` at the **application layer** around the generated tonic client call, before serialization. | At-least-once via `acks = "all"` + manual offset commit after successful processing |
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
}
```

### `GrpcClientConfig`

```rust
pub struct GrpcClientConfig {
    pub endpoint: String,                       // e.g. "https://svc:50051"
    pub tls: Option<GrpcTlsConfig>,             // None = plaintext (service-mesh mTLS at sidecar)
    pub connect_timeout: Duration,              // default: 5s
    pub resilience: Option<GrpcResilienceConfig>, // default: CircuitBreaker + 10s timeout
}
```

### `GrpcServerBuilder` / `TracedGrpcServer`

```rust
pub type TracedGrpcServer = Server<Stack<InboundTraceLayer, Identity>>;

pub struct GrpcServerBuilder { /* private */ }

impl GrpcServerBuilder {
    pub fn new(config: GrpcServerConfig) -> Self;
    // Returns a server with InboundTraceLayer pre-installed.
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
pub struct KafkaConsumerHandle { /* private — wraps StreamConsumer */ }

impl KafkaConsumerHandle {
    // Lazy async stream. Each item: trace context extracted + parent span set + payload deserialized.
    pub fn stream<T: ConsumablePayload>(
        &self,
    ) -> impl Stream<Item = Result<EventEnvelope<T>, TransportError>> + '_;

    // Commit offset asynchronously. Required when enable_auto_commit = false (default).
    pub fn commit<'a>(
        &'a self,
        msg: &BorrowedMessage<'a>,
    ) -> Result<(), TransportError>;
}
```

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

    // 2b. gRPC client — traced channel, add resilience layers on top
    use tower::ServiceBuilder;
    use resilience::{
        circuit_breaker::layer::CircuitBreakerLayer,
        timeout::layer::TimeoutLayer,
    };

    let channel = GrpcClientBuilder::new(
        GrpcClientConfig::new("https://dependency-svc:50051")
    )
    .build_traced()
    .await?;

    let svc = ServiceBuilder::new()
        .map_err(transport::TransportError::from_resilience)
        .layer(TimeoutLayer::new(resilience_cfg.timeout))
        .map_err(transport::TransportError::from_resilience_connect)
        .layer(CircuitBreakerLayer::new(resilience_cfg.circuit_breaker))
        .service(channel);

    let mut client = DependencyServiceClient::new(svc);

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

# Run all tests
cargo test -p transport

# Lint
cargo clippy -p transport -- -D warnings

# Format
cargo fmt -p transport
```

### Run a local Kafka broker (required for integration tests)

```bash
docker compose up -d kafka
# Kafka available at localhost:9092
```

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
