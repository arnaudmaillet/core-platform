# `cqrs` — Zero-overhead in-process Command/Query bus with typed middleware pipeline

## 🎯 Overview & Service Role

`cqrs` is the **application-dispatch layer** of the platform. It delivers a high-performance, fully static, in-process Command Bus and Query Bus that every microservice uses to route domain operations to their single registered handler — with no dynamic dispatch on the hot path.

**Critical problems solved:**

- **Strict write/read separation** — commands mutate state and return `()`. Queries return typed data and carry no side-effects. The type system enforces this contract at compile time; there is no way to write to state via the query path.
- **Type-safe routing without reflection** — handlers are registered by `TypeId` at startup. Dispatch is a single `HashMap::get` plus one heap allocation. The handler itself is called through a statically-compiled bridge closure, never via `dyn` vtable.
- **Composable cross-cutting concerns** — a Tower-inspired layer system wraps buses at the type level. The full middleware stack is a concrete type; every `dispatch` call is monomorphised end-to-end.
- **Distributed observability built-in** — every message carries an `Envelope<T>` with `message_id`, `correlation_id`, and `causation_id`, enabling end-to-end trace propagation from gRPC ingress through the bus and into handlers.

---

## 📐 Architecture & Concepts

### Layered data flow

```
gRPC / Kafka handler
        │
        │  Envelope::new(correlation_id, payload)
        ▼
┌───────────────────────────────────────────────────────────┐
│                  Middleware Pipeline                       │
│                                                           │
│  LoggingCommandBus         ← outermost (last .layer())   │
│    └─ TracingCommandBus    ← OTel span wraps inner call  │
│         └─ IdempotencyCommandBus  ← deduplicates by UUID │
│              └─ InMemoryCommandBus  ← TypeId→handler map │
│                   └─ TypedHandlerBridge<H, C>            │
│                        └─ Arc<H>.handle(envelope)        │
└───────────────────────────────────────────────────────────┘
        │
        ▼
   Result<(), CqrsError>  or  Result<Q::Response, CqrsError>
```

### Core concepts

| Concept | Description |
|---|---|
| **`Command`** | Marker trait. Represents intent to mutate state (imperative mood: `CreatePost`, `FollowUser`). Dispatched exactly once to one handler. |
| **`Query`** | Marker trait with associated `Response` type. Read-only. Dispatched to one handler; result is returned typed. |
| **`Envelope<T>`** | Transport wrapper. Carries `message_id` (idempotency key), `correlation_id` (trace thread), `causation_id` (causal parent), wall-clock `issued_at`, and an open `metadata` bag. |
| **`CommandBus` / `QueryBus`** | Abstract dispatch interface. Not object-safe (generic `dispatch<C>`). Concrete implementations are fully statically typed. |
| **`InMemoryCommandBus`** | `TypeId`-keyed `Arc<HashMap>` of type-erased handler bridges. Immutable after construction. `Clone`. |
| **`MiddlewarePipeline<S>`** | Builder that stacks `CommandLayer` / `QueryLayer` wrappers around a base bus. Each `.layer()` call is a type-level transformation — no runtime cost. |
| **`CqrsError`** | Bus-level error. Implements `AppError` and delegates all metadata fields to the original handler error via `BoxedDynAppError`. |

### Type erasure boundary

The only place `dyn` appears on the hot path is inside `ErasedCommandHandler` / `ErasedQueryHandler`, which are **sealed `pub(crate)` traits**. External code never names them. The `Arc<dyn ErasedCommandHandler>` stored in the registry downcasts back to the concrete envelope type inside a `Box::pin` closure; the downcast cannot fail because the `TypeId` key guarantees the match.

```
TypeId::of::<C>() ──→ Arc<dyn ErasedCommandHandler>
                            │ downcast Box<dyn Any + Send> → Envelope<C>  (infallible)
                            ▼
                       Arc<H>.handle(typed_envelope)   ← zero vtable dispatch
```

### Resilience guarantees & high-load behaviour

| Concern | Behaviour |
|---|---|
| **Registry lookup** | O(1) `HashMap::get` on an `Arc<HashMap>` — no lock contention; the map is immutable after `build()`. |
| **Allocation per dispatch** | One `Box::new(envelope)` to cross the type-erasure boundary. All middleware wrappers are zero-alloc (no `Box` or `dyn` in the wrapper chain). |
| **Handler panics** | Propagate normally. Wrap the bus in a `catch_unwind` layer or let Tokio's task boundary catch them if that is a requirement. |
| **Idempotency under concurrent load** | `InMemoryIdempotencyStore` uses `DashMap` — sharded, lock-free reads for the common "already processed" check. No global mutex. |
| **Idempotency on failure** | `mark_processed` is called **only on `Ok(())`**. A handler returning `Err` leaves the message unmarked; the caller can safely retry. |
| **Memory growth (idempotency store)** | `InMemoryIdempotencyStore` grows unbounded. For long-running services, replace it with a TTL-backed store (Redis `SET NX EX`). |
| **Backpressure** | This is an in-process bus. There is no queue and no buffer. Backpressure is provided by the caller's async scheduler (Tokio task capacity). |

---

## 🔌 Public Interfaces & API Contract

### `Envelope<T>`

```rust
pub struct Envelope<T> {
    pub message_id:     Uuid,                    // UUIDv7 — idempotency key
    pub correlation_id: Uuid,                    // propagated across the full request chain
    pub causation_id:   Option<Uuid>,            // message_id of the upstream trigger, if any
    pub issued_at:      DateTime<Utc>,           // wall-clock construction time (UTC)
    pub metadata:       HashMap<String, String>, // open bag: tenant_id, OTel bytes, flags
    pub payload:        T,
}

impl<T> Envelope<T> {
    // Starts a fresh causal chain (use at the gRPC / Kafka ingress boundary).
    pub fn new(correlation_id: Uuid, payload: T) -> Self;

    // Continues a chain: inherits parent correlation_id, sets causation_id = parent.message_id,
    // and clones parent metadata (tenant_id etc. flow through automatically).
    pub fn new_caused_by<P>(parent: &Envelope<P>, payload: T) -> Self;

    // Builder-style metadata attachment.
    pub fn with_metadata(self, key: impl Into<String>, value: impl Into<String>) -> Self;

    // Transforms payload; all envelope fields are preserved.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Envelope<U>;
}
```

### `Command` + `CommandHandler<C>` + `CommandBus`

```rust
/// Marker trait — name in the imperative mood.
pub trait Command: Send + Sync + 'static {}

/// One handler per command type. Error must implement AppError.
pub trait CommandHandler<C: Command>: Send + Sync + 'static {
    type Error: AppError;
    fn handle(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send + '_;
}

/// Dispatch entry point. Not object-safe (generic dispatch<C>).
pub trait CommandBus: Send + Sync {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_;
}
```

### `Query` + `QueryHandler<Q>` + `QueryBus`

```rust
/// Marker trait with associated typed response.
pub trait Query: Send + Sync + 'static {
    type Response: Send + Sync + 'static;
}

/// One handler per query type. Returns Q::Response on success.
pub trait QueryHandler<Q: Query>: Send + Sync + 'static {
    type Error: AppError;
    fn handle(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, Self::Error>> + Send + '_;
}

/// Dispatch entry point. Returns Q::Response on success.
pub trait QueryBus: Send + Sync {
    fn dispatch<Q: Query>(
        &self,
        envelope: Envelope<Q>,
    ) -> impl Future<Output = Result<Q::Response, CqrsError>> + Send + '_;
}
```

### `CqrsError`

```rust
#[derive(Debug)]
pub enum CqrsError {
    /// No handler registered for this type. Programming error — never in production.
    HandlerNotFound { type_name: &'static str },

    /// Duplicate registration caught at bus construction time.
    DuplicateRegistration { type_name: &'static str },

    /// Handler returned an error. All AppError metadata (error_code, http_status,
    /// severity, is_retryable, category, user_facing_message) is preserved and
    /// accessible directly on CqrsError via its own AppError impl.
    Handler(BoxedDynAppError),
}
```

`CqrsError` implements `AppError`. Calling `e.error_code()` on a `CqrsError::Handler` variant returns the original handler error code transparently.

| Variant | `error_code` | `http_status` | `is_retryable` |
|---|---|---|---|
| `HandlerNotFound` | `CQRS_HANDLER_NOT_FOUND` | 500 | `false` |
| `DuplicateRegistration` | `CQRS_DUPLICATE_REGISTRATION` | 500 | `false` |
| `Handler(e)` | delegates to `e` | delegates to `e` | delegates to `e` |

### Middleware extension point

```rust
pub trait CommandLayer<S> {
    type Service;
    fn layer(&self, inner: S) -> Self::Service;
}

pub trait QueryLayer<S> {
    type Service;
    fn layer(&self, inner: S) -> Self::Service;
}
```

Implement both (or one) to add custom middleware. The `Service` type must itself implement `CommandBus` or `QueryBus`.

### Bundled layers

| Layer | Trait impls | Behaviour |
|---|---|---|
| `TracingLayer` | `CommandLayer` + `QueryLayer` | Opens an `info_span!` per dispatch. Fields: `otel.kind=INTERNAL`, `message.type`, `message.id`, `correlation.id`. Span is active across all `await` points including handler execution. |
| `LoggingLayer` | `CommandLayer` + `QueryLayer` | Emits `tracing::info!` on start and `info!`/`error!` on completion. Adds `elapsed_ms` and `error.code` on failure. |
| `IdempotencyLayer<Store>` | `CommandLayer` only | Deduplicates by `envelope.message_id`. Pluggable backend via `IdempotencyStore`. Ships `InMemoryIdempotencyStore` (DashMap). |

### `IdempotencyStore` trait

```rust
pub trait IdempotencyStore: Send + Sync + 'static {
    fn is_processed(&self, message_id: Uuid) -> impl Future<Output = bool> + Send + '_;
    fn mark_processed(&self, message_id: Uuid) -> impl Future<Output = ()> + Send + '_;
}
```

Replace `InMemoryIdempotencyStore` with a Redis or ScyllaDB implementation for multi-replica deployments.

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# In your service's Cargo.toml
[dependencies]
cqrs = { path = "../../shared/cqrs" }
```

### Step 1 — Define a command and its handler

```rust
use cqrs::{Command, CommandHandler, Envelope};
use error::AppError;

pub struct CreatePostCommand {
    pub title: String,
}
impl Command for CreatePostCommand {}

pub struct CreatePostHandler {
    repo: Arc<PostRepository>,
}

impl CommandHandler<CreatePostCommand> for CreatePostHandler {
    type Error = PostServiceError; // must implement AppError

    async fn handle(
        &self,
        envelope: Envelope<CreatePostCommand>,
    ) -> Result<(), PostServiceError> {
        self.repo
            .insert(envelope.payload.title, envelope.correlation_id)
            .await
    }
}
```

### Step 2 — Define a query and its handler

```rust
use cqrs::{Query, QueryHandler, Envelope};

pub struct GetPostByIdQuery { pub id: Uuid }
impl Query for GetPostByIdQuery {
    type Response = PostDto;
}

pub struct GetPostByIdHandler { read_db: Arc<ReadDatabase> }

impl QueryHandler<GetPostByIdQuery> for GetPostByIdHandler {
    type Error = PostServiceError;

    async fn handle(
        &self,
        envelope: Envelope<GetPostByIdQuery>,
    ) -> Result<PostDto, PostServiceError> {
        self.read_db.find_post(envelope.payload.id).await
    }
}
```

### Step 3 — Build and decorate buses at application startup

```rust
use cqrs::{
    CommandBusBuilder, QueryBusBuilder, MiddlewarePipeline,
    IdempotencyLayer, InMemoryIdempotencyStore, TracingLayer, LoggingLayer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize telemetry FIRST — TracingLayer reads the global OTel subscriber.
    let _guard = telemetry::init(TelemetryConfig::from_env(
        "post-command-server",
        env!("CARGO_PKG_VERSION"),
    ))?;

    // Build the raw command bus (fails fast on duplicate registration).
    let raw_cmd_bus = CommandBusBuilder::new()
        .register::<CreatePostCommand, _>(CreatePostHandler { repo: repo.clone() })?
        .register::<DeletePostCommand, _>(DeletePostHandler { repo })?
        .build(); // → InMemoryCommandBus (Arc-backed, Clone, immutable)

    // Decorate with middleware. First .layer() = outermost = runs first.
    // Final concrete type (fully static, zero dyn):
    //   LoggingCommandBus<TracingCommandBus<IdempotencyCommandBus<InMemoryCommandBus>>>
    let command_bus = MiddlewarePipeline::new(raw_cmd_bus)
        .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
        .layer(TracingLayer)
        .layer(LoggingLayer)
        .build();

    // Build the query bus (no IdempotencyLayer — queries are naturally idempotent).
    let query_bus = MiddlewarePipeline::new(
        QueryBusBuilder::new()
            .register::<GetPostByIdQuery, _>(GetPostByIdHandler { read_db })?
            .build(),
    )
    .query_layer(TracingLayer)
    .query_layer(LoggingLayer)
    .build();

    // Hand buses to your gRPC service and serve.
    Ok(())
}
```

### Step 4 — Dispatch from a gRPC endpoint

```rust
use cqrs::{CommandBus, Envelope};

async fn grpc_create_post(
    bus: &impl CommandBus,
    req: CreatePostRequest,
    correlation_id: Uuid,   // extracted from gRPC metadata
) -> Result<(), CqrsError> {
    let envelope = Envelope::new(correlation_id, CreatePostCommand { title: req.title });
    bus.dispatch(envelope).await
}
```

### Causal chaining (command → follow-on command)

```rust
// Inside a handler that triggers a downstream command:
let child_envelope = Envelope::new_caused_by(
    &incoming_envelope,                      // inherits correlation_id + metadata
    PublishNotificationCommand { user_id },  // new message_id generated automatically
);
bus.dispatch(child_envelope).await?;
```

### Custom middleware

```rust
pub struct AuthorizationLayer { policy: Arc<PolicyEngine> }

pub struct AuthorizationCommandBus<S> {
    inner: S,
    policy: Arc<PolicyEngine>,
}

impl<S> CommandLayer<S> for AuthorizationLayer {
    type Service = AuthorizationCommandBus<S>;
    fn layer(&self, inner: S) -> Self::Service {
        AuthorizationCommandBus { inner, policy: Arc::clone(&self.policy) }
    }
}

impl<S: CommandBus> CommandBus for AuthorizationCommandBus<S> {
    fn dispatch<C: Command>(
        &self,
        envelope: Envelope<C>,
    ) -> impl Future<Output = Result<(), CqrsError>> + Send + '_ {
        async move {
            self.policy.check(&envelope.metadata).await?;
            self.inner.dispatch(envelope).await
        }
    }
}
```

---

## ⚙️ Configuration & Runtime Environment

`cqrs` is a **pure in-process library** — it reads no environment variables and has no network I/O. Configuration is entirely code-driven at bus construction time.

| Variable | Required | Default | Description |
|---|---|---|---|
| _(none)_ | — | — | This crate has no runtime environment variables. |

**Application-level prerequisites (not this crate's concern):**

- `telemetry::init()` must be called before constructing any bus that uses `TracingLayer` or `LoggingLayer`. Without it, `tracing` events are emitted but discarded (no-op subscriber).
- For distributed idempotency, replace `InMemoryIdempotencyStore` with a Redis-backed `IdempotencyStore` impl. The `fred` crate is available in the workspace.

**Cargo features:**

There are no optional Cargo features in this crate. All components are compiled unconditionally.

---

## 📈 Telemetry, Performance & Metrics

### Runtime prerequisites

- **Async runtime:** Tokio (multi-threaded scheduler). All futures are `Send + 'static`; they are compatible with `tokio::spawn`.
- **OTel subscriber:** Must be installed by the calling application (via `telemetry::init()`) before the first `dispatch` call that uses `TracingLayer`.

### OTel span fields (`TracingLayer`)

Span name: `cqrs.command.dispatch` or `cqrs.query.dispatch`

| Field | Value |
|---|---|
| `otel.kind` | `"INTERNAL"` |
| `message.type` | Fully qualified Rust type name (e.g. `post::command::CreatePostCommand`) |
| `message.id` | UUIDv7 of the envelope — unique per dispatch |
| `correlation.id` | Propagated `correlation_id` — shared across all messages in one request |

### Structured log fields (`LoggingLayer`)

**Start event** (`cqrs.command dispatch started`):

| Field | Value |
|---|---|
| `message.type` | Fully qualified Rust type name |
| `correlation_id` | Envelope's `correlation_id` |
| `message_id` | Envelope's `message_id` |

**Completion event** (`cqrs.command dispatch completed` / `cqrs.command dispatch failed`):

| Field | Value |
|---|---|
| All start fields | (same as above) |
| `elapsed_ms` | Wall-clock handler time in milliseconds |
| `error` | Error display string (failure only) |
| `error.code` | `AppError::error_code()` value (failure only) |

### Performance profile

| Operation | Cost |
|---|---|
| `CommandBus::dispatch` (hot path) | 1× `HashMap::get` (O(1), no lock) + 1× `Box::new(envelope)` heap allocation |
| `QueryBus::dispatch` (hot path) | Same as above + 1× `Box::new(response)` + downcast on return |
| Middleware chain overhead | Zero allocation per layer — all wrappers are stack-allocated `async` closures |
| Bus clone | `Arc::clone` — O(1), no deep copy |

### Recommended production alerts

| Alert | Condition | Severity |
|---|---|---|
| High command dispatch latency | `cqrs.command.dispatch` p99 > 50 ms | Warning |
| Handler error rate | `cqrs.command.dispatch` error rate > 1% | Critical |
| Idempotency store memory | Process RSS growth without bound (sign of unbounded `InMemoryIdempotencyStore`) | Warning |

---

## 🛠️ Local Development & Contribution

### Build

```bash
# From workspace root
cargo build -p cqrs

# Or from the crate directory
cargo build
```

### Format & lint

```bash
cargo fmt --package cqrs
cargo clippy --package cqrs -- -D warnings
```

### Test

```bash
# Unit tests (no external dependencies)
cargo test --package cqrs

# With output
cargo test --package cqrs -- --nocapture
```

### Local dev dependencies

None. This crate has no I/O, no network calls, and no required external services. All tests run fully in-process.

### Adding a new bundled middleware layer

1. Create `src/middleware/my_layer.rs`.
2. Define `MyLayer` (unit struct or config struct) and `MyCommandBus<S>` / `MyQueryBus<S>`.
3. Implement `CommandLayer<S>` and/or `QueryLayer<S>` for `MyLayer`.
4. Implement `CommandBus` and/or `QueryBus` for `MyCommandBus<S>` / `MyQueryBus<S>`.
5. Add `pub(crate) mod my_layer;` and `pub use my_layer::*;` to `src/middleware/mod.rs`.

**Key invariants to preserve:**
- No `async_trait` macro — use native RPIT (`fn foo() -> impl Future<...> + Send + '_`).
- `BoxFuture` only if you need object safety for a `dyn` trait (i.e. a new erased bridge). Avoid it in middleware wrappers.
- The `dispatch` body must be a single `async move {}` block or an `.instrument(span)` call — never allocate inside the wrapper for the common path.

---

## 🚨 Troubleshooting & Runbook (FAQ)

### 1. `CqrsError::HandlerNotFound` at runtime

**Symptom:** `no handler registered for \`my_service::command::CreatePostCommand\`` returned from `dispatch`.

**Root cause:** The command type was never passed to `CommandBusBuilder::register` before `.build()` was called. Common causes: a handler registration line was accidentally omitted, or the bus was built in a different scope than where the command is dispatched.

**Fix:** Audit the `CommandBusBuilder` chain at your service's startup. All command types that will ever be dispatched must be registered before `build()`. Consider adding a startup smoke-test that dispatches a no-op probe command.

---

### 2. `CqrsError::DuplicateRegistration` at startup

**Symptom:** Service fails to start with `handler already registered for \`...\``.

**Root cause:** `CommandBusBuilder::register::<C, _>(...)` was called twice with the same `C`. This is caught eagerly at builder time, not at dispatch time.

**Fix:** Search your startup code for the command type name. Remove the duplicate registration. If two handlers are legitimately needed (e.g., fan-out), route through a single aggregating handler that delegates to both.

---

### 3. `InMemoryIdempotencyStore` memory grows unbounded in long-running services

**Symptom:** Process RSS climbs steadily over hours or days. Heap profiling points to the `DashMap` inside `InMemoryIdempotencyStore`.

**Root cause:** `InMemoryIdempotencyStore` never evicts entries. Every processed `message_id` (a 16-byte UUID) is retained forever.

**Mitigation:**
- **Short-term:** Restart the service on a regular cadence (acceptable for stateless replicas).
- **Long-term:** Implement `IdempotencyStore` backed by Redis using `SET NX EX <ttl>`. A 24-hour TTL matches typical at-least-once delivery windows. Switch the bus at startup:

```rust
let store = RedisIdempotencyStore::new(redis_client, Duration::from_secs(86400));
let bus = MiddlewarePipeline::new(raw_bus)
    .layer(IdempotencyLayer::new(store))
    // ...
    .build();
```
