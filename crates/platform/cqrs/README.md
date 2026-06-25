# `cqrs` — Zero-overhead in-process Command/Query bus with a typed middleware pipeline

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `platform` — the application-dispatch layer (static, in-process) |
> | **Package** | `cqrs` (dir: `crates/platform/cqrs`) |
> | **Consumed by** | every service (command/query buses); `validation` & `auth-context` extend it |
> | **Depends on** | `error`, `validate-core`, `uuid`, `chrono`, `dashmap`, `tracing` |
> | **Stability** | stable contract |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`cqrs` is the application-dispatch layer: a high-performance, fully static, in-process **Command Bus**
and **Query Bus** that route domain operations to their single registered handler with **no dynamic
dispatch on the hot path**. Every message rides an `Envelope<T>` carrying `message_id` /
`correlation_id` / `causation_id` for end-to-end trace propagation.

**Architectural boundary** — strict write/read separation enforced by the type system: commands mutate
and return `()`; queries return typed data and carry no side-effects. There is no way to write state
through the query path. It is a pure in-process bus — no queue, no network, no env.

---

## 📐 Architecture & key decisions

```
gRPC/Kafka handler ─► Envelope::new(correlation_id, payload)
  ▼ LoggingCommandBus (outermost) ─► TracingCommandBus ─► IdempotencyCommandBus
    ─► InMemoryCommandBus (TypeId→handler map) ─► TypedHandlerBridge<H,C> ─► Arc<H>.handle(envelope)
  ▼ Result<(), CqrsError>   (queries: Result<Q::Response, CqrsError>)
```

- **TypeId routing, no reflection, no vtable** — handlers register by `TypeId` at startup; dispatch is
  one `HashMap::get` + one `Box::new(envelope)` to cross the erasure boundary, then the handler runs
  through a statically-compiled bridge. The only `dyn` is inside **sealed `pub(crate)`**
  `ErasedCommandHandler`/`ErasedQueryHandler`; the downcast can't fail because the `TypeId` key
  guarantees the type.
- **No `async_trait`** — handlers/buses use native RPIT (`-> impl Future<…> + Send + '_`), so the
  pipeline is monomorphised end-to-end with no boxed futures in the wrapper chain. `BoxFuture` appears
  **only** in the erased bridges.
- **`pub(crate)` middleware modules + targeted `pub use`** — avoids glob-collision between the command
  and query layer types while keeping the public names clean.
- **Idempotency marks only on `Ok`** — `mark_processed` runs only on success, so a failed handler
  leaves the message unmarked and safely retryable. `InMemoryIdempotencyStore` (DashMap) is
  unbounded — swap for a TTL store (Redis `SET NX EX`) in long-running services.

---

## 🔌 Public API & contract

```rust
pub struct Envelope<T> { pub message_id: Uuid, pub correlation_id: Uuid, pub causation_id: Option<Uuid>,
                         pub issued_at: DateTime<Utc>, pub metadata: HashMap<String,String>, pub payload: T }
impl<T> Envelope<T> {
    pub fn new(correlation_id: Uuid, payload: T) -> Self;            // fresh causal chain (ingress)
    pub fn new_caused_by<P>(parent: &Envelope<P>, payload: T) -> Self; // inherits correlation + metadata, sets causation
    pub fn with_metadata(self, k: impl Into<String>, v: impl Into<String>) -> Self;
    pub fn map<U, F: FnOnce(T)->U>(self, f: F) -> Envelope<U>;
}

pub trait Command: Send + Sync + 'static {}                          // (supertrait: validate_core::Validate)
pub trait Query:   Send + Sync + 'static { type Response: Send + Sync + 'static; }
pub trait CommandHandler<C: Command>: Send + Sync + 'static { type Error: AppError; fn handle(&self, e: Envelope<C>) -> impl Future<Output=Result<(), Self::Error>> + Send + '_; }
pub trait QueryHandler<Q: Query>:     Send + Sync + 'static { type Error: AppError; fn handle(&self, e: Envelope<Q>) -> impl Future<Output=Result<Q::Response, Self::Error>> + Send + '_; }
pub trait CommandBus: Send + Sync { fn dispatch<C: Command>(&self, e: Envelope<C>) -> impl Future<Output=Result<(), CqrsError>> + Send + '_; }       // not object-safe
pub trait QueryBus:   Send + Sync { fn dispatch<Q: Query>(&self, e: Envelope<Q>)   -> impl Future<Output=Result<Q::Response, CqrsError>> + Send + '_; }

pub enum CqrsError { HandlerNotFound { type_name: &'static str }, DuplicateRegistration { type_name: &'static str }, Handler(BoxedDynAppError) }
// impl AppError — Handler(e) delegates error_code/http_status/severity/… to the original handler error.

pub trait CommandLayer<S> { type Service; fn layer(&self, inner: S) -> Self::Service; }   // + QueryLayer<S>
pub trait IdempotencyStore: Send + Sync + 'static { fn is_processed(&self, id: Uuid) -> impl Future<Output=bool>+Send+'_; fn mark_processed(&self, id: Uuid) -> impl Future<Output=()>+Send+'_; }
```

Bundled layers: `TracingLayer` (`info_span!` per dispatch), `LoggingLayer` (start/complete + `elapsed_ms`/`error.code`),
`IdempotencyLayer<Store>` (command-only, dedup by `message_id`). `CqrsError` codes:
`HandlerNotFound`→`CQRS_HANDLER_NOT_FOUND`/500, `DuplicateRegistration`→`CQRS_DUPLICATE_REGISTRATION`/500,
`Handler(e)`→delegates.

> **Contract notes:** the dispatch traits are **not object-safe** (generic `dispatch<C>`) — hold the
> concrete bus type (or its `Arc`). The final decorated bus is a concrete type, e.g.
> `LoggingCommandBus<TracingCommandBus<IdempotencyCommandBus<InMemoryCommandBus>>>`.

---

## 📦 Integration

```toml
[dependencies]
cqrs = { workspace = true }
```

```rust
// build at startup — register (fails fast on duplicates), then decorate; FIRST .layer() = outermost.
let raw = CommandBusBuilder::new()
    .register::<CreatePostCommand, _>(CreatePostHandler { repo })?
    .build();                                   // InMemoryCommandBus (Arc, Clone, immutable)
let command_bus = MiddlewarePipeline::new(raw)
    .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
    .layer(TracingLayer).layer(LoggingLayer).build();

// dispatch from a gRPC endpoint:
bus.dispatch(Envelope::new(correlation_id, CreatePostCommand { title })).await?;
// causal chaining inside a handler:
bus.dispatch(Envelope::new_caused_by(&incoming, PublishNotificationCommand { user_id })).await?;
```

Queries follow the same shape with `QueryBusBuilder` + `.query_layer(...)` (no `IdempotencyLayer` —
queries are naturally idempotent). Custom middleware = implement `CommandLayer<S>`/`QueryLayer<S>` and
`CommandBus`/`QueryBus` for your wrapper.

---

## ⚙️ Configuration & feature flags

Pure in-process library — no env vars, no cargo features. Prerequisite (not this crate's concern):
`telemetry::init()` before the first dispatch that uses `TracingLayer`/`LoggingLayer`, else events are
discarded.

---

## 🔭 Observability

`TracingLayer` span `cqrs.command.dispatch` / `cqrs.query.dispatch`: `otel.kind=INTERNAL`,
`message.type`, `message.id`, `correlation.id`. `LoggingLayer`: start + complete/failed with
`elapsed_ms`, `error`, `error.code`. Hot path = 1× `HashMap::get` (O(1), no lock) + 1× `Box::new`;
bus clone = `Arc::clone`.

Suggested alerts: `cqrs.command.dispatch` p99 > 50ms ⇒ warn; handler error rate > 1% ⇒ critical;
unbounded RSS growth ⇒ warn (unbounded `InMemoryIdempotencyStore`).

---

## 🧪 Testing

```bash
cargo test   -p cqrs                 # fully in-process, no external deps
cargo clippy -p cqrs --all-targets
```

When adding a bundled layer, preserve the engine invariants: **no `async_trait`** (use native RPIT);
`BoxFuture` only for a new erased bridge, never in middleware wrappers; the `dispatch` body is a single
`async move {}` / `.instrument(span)` with no allocation on the common path.

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `CqrsError::HandlerNotFound` at runtime.**
The command type was never `register`ed before `.build()` (omitted line, or the bus was built in a
different scope than where you dispatch). Audit the builder chain; consider a startup smoke-test that
dispatches a probe command.

**2. `CqrsError::DuplicateRegistration` at startup.**
`register::<C, _>` was called twice for the same `C` (caught eagerly at build time). Remove the
duplicate; for legitimate fan-out, route through one aggregating handler.

**3. RSS grows unbounded over days.**
`InMemoryIdempotencyStore` never evicts (every 16-byte `message_id` is retained). Restart on a cadence
(stateless replicas) or implement `IdempotencyStore` over Redis `SET NX EX <ttl>` (~24h matches typical
at-least-once windows) and swap it in at startup.

**4. I tried to store a `&dyn CommandBus` and it won't compile.**
The dispatch traits aren't object-safe (`dispatch<C>` is generic). Hold the concrete decorated bus type
or its `Arc` instead of a trait object.
