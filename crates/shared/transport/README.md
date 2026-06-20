# `transport` — Interface Contract

## Responsibility

Provide the single, platform-wide communication layer for the hyperscale social network backend. It orchestrates:

- **Synchronous RPC** (gRPC via Tonic + Tower) with circuit-breaker, timeout, and automatic W3C TraceContext injection.
- **Asynchronous event-driven messaging** (Kafka via rdkafka) with automatic W3C TraceContext injection on produce and extraction on consume.
- **Payload serialization** (JSON and Protobuf codecs) shared across both transports.

This crate owns **no business logic**. It is a pure infrastructure library that every microservice depends on.

---

## Directory Layout

```
src/
├── lib.rs
├── error.rs                        TransportError + CodecError (top-level enums)
├── propagation/
│   ├── carrier.rs                  MetadataCarrier trait + inject_context / extract_context
│   ├── grpc.rs                     GrpcHeaderInjector / GrpcHeaderExtractor (http::HeaderMap)
│   └── kafka.rs                    KafkaHeaderInjector / KafkaHeaderExtractor (OwnedHeaders / BorrowedHeaders)
├── codec/
│   ├── json.rs                     encode<T: Serialize> / decode<T: DeserializeOwned>
│   └── protobuf.rs                 encode<M: Message> / decode<M: Message + Default>
├── grpc/
│   ├── error.rs                    GrpcTransportError + grpc_severity()
│   ├── layer/
│   │   ├── outbound.rs             OutboundTraceLayer — injects trace into outgoing headers
│   │   └── inbound.rs              InboundTraceLayer  — extracts trace from incoming headers
│   ├── client/
│   │   ├── config.rs               GrpcClientConfig + GrpcResilienceConfig + GrpcTlsConfig
│   │   └── builder.rs              GrpcClientBuilder → GrpcClientService (BoxService)
│   └── server/
│       ├── config.rs               GrpcServerConfig + GrpcServerTlsConfig
│       └── builder.rs              GrpcServerBuilder → tonic::Server (with InboundTraceLayer)
└── kafka/
    ├── error.rs                    KafkaTransportError
    ├── envelope.rs                 EventEnvelope<T> + PublishablePayload + ConsumablePayload
    ├── config/
    │   ├── client.rs               KafkaClientConfig (shared broker / SASL settings)
    │   ├── producer.rs             ProducerConfig (acks, compression, linger_ms)
    │   └── consumer.rs             ConsumerConfig (group_id, auto_offset_reset)
    ├── producer/
    │   ├── builder.rs              KafkaProducerBuilder → KafkaProducerHandle
    │   └── handle.rs               KafkaProducerHandle — .publish<T>() / .publish_raw()
    └── consumer/
        ├── builder.rs              KafkaConsumerBuilder → KafkaConsumerHandle
        └── handle.rs               KafkaConsumerHandle — .stream<T>() / .commit()
```

---

## Public Interfaces

### `TransportError` — unified error type

```rust
pub enum TransportError {
    Grpc(GrpcTransportError),         // tonic::transport::Error or tonic::Status
    Kafka(KafkaTransportError),       // rdkafka::error::KafkaError
    Codec(CodecError),                // serde_json / prost encode/decode
    CircuitOpen,                      // resilience::CircuitBreakerLayer rejected the call
    Timeout(Duration),                // resilience::TimeoutLayer deadline exceeded
    MaxRetriesExhausted(u32),         // resilience::RetryLayer (application-level callers)
}
```

`TransportError` implements `From<tonic::transport::Error>`, `From<tonic::Status>`, and `From<GrpcTransportError>` / `From<KafkaTransportError>` / `From<CodecError>` for ergonomic `?` propagation.

---

### `propagation` module

```rust
// Inject the current tracing span's context into any carrier (gRPC headers / Kafka headers)
pub fn inject_context<C: Injector>(carrier: &mut C);

// Extract a remote context from any carrier
pub fn extract_context<C: Extractor>(carrier: &C) -> opentelemetry::Context;

// Wire a remote context as the parent of a local span
pub fn set_parent(span: &tracing::Span, cx: opentelemetry::Context);
```

Carrier implementations:

| Carrier | Transport | Direction |
|---------|-----------|-----------|
| `GrpcHeaderInjector<'_>` | gRPC | Outbound (mutates `&mut http::HeaderMap`) |
| `GrpcHeaderExtractor<'_>` | gRPC | Inbound (reads `&http::HeaderMap`) |
| `KafkaHeaderInjector` | Kafka | Outbound (owns `OwnedHeaders`) |
| `KafkaHeaderExtractor<'_>` | Kafka | Inbound (reads `&BorrowedHeaders`) |

All four implement `opentelemetry::propagation::Injector` / `Extractor` and thus satisfy the `MetadataCarrier` supertrait.

---

### gRPC — client side

```rust
let config = GrpcClientConfig::new("https://post-command-server:50051");

// Fully-composed service: OutboundTrace + CircuitBreaker + Timeout, errors → TransportError
let svc: GrpcClientService = GrpcClientBuilder::new(config).build().await?;
let mut client = PostServiceClient::new(svc);

// Or: raw channel (cloneable, no middleware pre-applied)
let channel: tonic::transport::Channel = GrpcClientBuilder::new(config).connect().await?;
```

**Stack composition (outermost → innermost):**

```text
TimeoutLayer
  └─ map_err → TransportError
      └─ CircuitBreakerLayer
          └─ map_err → TransportError
              └─ OutboundTraceLayer   (injects traceparent / tracestate)
                  └─ tonic::transport::Channel
```

**Why no `RetryLayer` at the transport level?**

HTTP/2 request bodies are streams. Once consumed they cannot be replayed without buffering the entire payload upfront, which is prohibitive. Apply `RetryLayer` at the **application layer** (around the generated gRPC client call, before serialization), not around the channel.

---

### gRPC — server side

```rust
let server = GrpcServerBuilder::new(GrpcServerConfig::default())
    .build()?                              // pre-installs InboundTraceLayer
    .add_service(PostServiceServer::new(my_handler))
    .serve(config.addr)
    .await?;
```

**Stack:**

```text
tonic::Server
  └─ InboundTraceLayer
      ├─ extracts traceparent / tracestate from request headers
      ├─ reconstructs remote OpenTelemetry context
      ├─ sets it as parent of a new `grpc.server` span
      └─ wraps the entire handler invocation inside that span
```

---

### Kafka — producer

```rust
let handle: KafkaProducerHandle = KafkaProducerBuilder::new(
    ProducerConfig::new(KafkaClientConfig::from_env())
).build()?;

let envelope = EventEnvelope::new("posts.created", post_id.to_string(), PostCreatedEvent { ... })
    .with_header("x-source-service", "post-command-server");

handle.publish(envelope).await?;
// traceparent + tracestate automatically injected into Kafka record headers.
```

---

### Kafka — consumer

```rust
let handle: KafkaConsumerHandle = KafkaConsumerBuilder::new(
    ConsumerConfig::new(KafkaClientConfig::from_env(), "profile-projection-consumer")
)
.subscribe("posts.created")
.build()?;

let mut stream = handle.stream::<PostCreatedEvent>();
while let Some(result) = stream.next().await {
    let envelope = result?;
    // Current span is already linked to the producer's trace via extracted traceparent.
    process(envelope.payload).await?;
}
```

---

### `EventEnvelope<T>`

```rust
pub struct EventEnvelope<T> {
    pub topic: String,
    pub key: String,        // Kafka partition key — use a stable domain ID for ordering
    pub payload: T,
    pub headers: HashMap<String, String>,  // user headers (trace headers are transport-internal)
    pub timestamp_ms: Option<i64>,
}
```

---

### `codec` module

Both functions return `Result<_, CodecError>`, which maps to `TransportError::Codec` via `From`.

```rust
// JSON
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, CodecError>;
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError>;

// Protobuf
pub fn encode<M: prost::Message>(msg: &M) -> Result<Bytes, CodecError>;
pub fn decode<M: prost::Message + Default>(bytes: &[u8]) -> Result<M, CodecError>;
```

---

## Integration with sibling crates

| Crate | How this crate uses it |
|-------|----------------------|
| `error` | `GrpcTransportError::grpc_severity()` maps `tonic::Code` to `error::Severity` for structured logging |
| `resilience` | `CircuitBreakerLayer` and `TimeoutLayer` are composed inside `GrpcClientBuilder::build()`; `TransportError::from_resilience*` helpers flatten `ResilienceError<E>` |
| `telemetry` | Must be initialised (`telemetry::init()`) before any transport call; the transport layer reads from and writes to the **global** OTel propagator installed by `telemetry::init()` |

---

## Required runtime environment

No environment variables are **required** — all have safe defaults.

| Variable | Default | Description |
|----------|---------|-------------|
| `KAFKA_BROKERS` | `localhost:9092` | Broker addresses (used by `KafkaClientConfig::from_env()`) |
| `KAFKA_SECURITY_PROTOCOL` | `PLAINTEXT` | `PLAINTEXT`, `SASL_SSL`, etc. |
| `KAFKA_SASL_MECHANISM` | — | `PLAIN`, `SCRAM-SHA-256` |
| `KAFKA_SASL_USERNAME` | — | |
| `KAFKA_SASL_PASSWORD` | — | |

OpenTelemetry configuration is inherited from the `telemetry` crate (same env vars: `OTEL_EXPORTER_OTLP_ENDPOINT`, etc.).

---

## Adding to a service

```toml
# service Cargo.toml
transport = { path = "crates/shared/transport" }
```

**Bootstrap sequence every binary must follow:**

```rust
#[tokio::main]
async fn main() {
    // 1. Init telemetry FIRST — registers the global OTel propagator that
    //    transport reads for trace injection / extraction.
    let _guard = telemetry::init(TelemetryConfig::from_env(
        "my-service", env!("CARGO_PKG_VERSION")
    )).expect("telemetry init failed");

    // 2. Build transport clients / servers (they read from the global propagator).
    let svc = GrpcClientBuilder::new(GrpcClientConfig::new("https://dep:50051"))
        .build()
        .await
        .expect("grpc client build failed");

    // 3. Use clients — trace context flows automatically.
}
```

---

## Key invariants

1. `telemetry::init()` must be called before the first `inject_context` or `extract_context` call; without the global propagator both are no-ops.
2. `KafkaProducerHandle` is `Clone` — it wraps an `Arc`-backed `FutureProducer` internally via rdkafka.
3. `GrpcClientService` (`BoxService`) is **not** `Clone`. If you need a cloneable gRPC client, use `GrpcClientBuilder::connect()` and apply layers manually.
4. `InboundTraceLayer` and `OutboundTraceLayer` carry no state — they are `Clone + Default` and can be applied to multiple servers / channels.
5. Consumer offset commitment is the caller's responsibility when `enable_auto_commit = false` (the default).
