# `cqrs` — Domain & Functional Contract

> The in-process Command/Query bus: a zero-overhead dispatch layer answering *"which single handler owns this operation, and how does its causal context propagate?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Application-dispatch: a static, in-process Command Bus + Query Bus with a typed middleware pipeline |
> | **Layer** | `platform` — the application-dispatch layer between transport handlers and domain handlers |
> | **Subdomain class** | **Generic** — a CQRS bus; leverage is the zero-dispatch hot path + uniform middleware |
> | **Primary abstraction(s)** | `Command`/`Query` + `Envelope<T>` + `CommandBus`/`QueryBus` (`cqrs`) |
> | **Footprint** | pure (in-process, no queue, no network, no env) |
> | **Failure posture** | N/A — it routes; failures are the handler's, surfaced as `CqrsError` |
> | **Depends on** | `error`, `validate-core`, `uuid`, `chrono`, `dashmap`, `tracing` |
> | **Consumed by** | every service (command/query buses); `validation` & `auth-context` extend it |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `cqrs` is the fleet's authority for **application dispatch**: it answers
**"which single registered handler owns this command/query, and how do `correlation_id`/`causation_id`
travel end-to-end?"** — with no dynamic dispatch on the hot path.

**The hard problem.** A bus that routes by type usually means reflection or a vtable on every call. `cqrs`
routes by `TypeId` (one `HashMap::get` + one `Box::new` to cross a *sealed* erasure boundary), keeps
handlers monomorphised via native RPIT (no `async_trait`), and enforces write/read separation in the type
system — commands mutate and return `()`, queries return data and carry no side-effects.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Be a message queue / network bus → it is purely in-process.
- ❌ Own the validation *rule* → `Validate` lives in `validate-core`; the middleware in `validation`.
- ❌ Persist idempotency durably → the bundled `InMemoryIdempotencyStore` is unbounded; swap a TTL store.
- ❌ Own tracing/telemetry init → it emits spans/logs but `telemetry::init()` is a prerequisite.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Command / Query | A write (returns `()`) / a read (returns typed data) | `Command`, `Query` |
| Handler | The single owner of a command/query type | `CommandHandler`, `QueryHandler` |
| Envelope | The message carrier with the causal chain | `Envelope<T>` (`message_id`/`correlation_id`/`causation_id`) |
| Bus | The dispatcher routing to a handler | `CommandBus`, `QueryBus`, `InMemoryCommandBus` |
| Layer / pipeline | Middleware wrapping dispatch | `CommandLayer`, `MiddlewarePipeline`, `Tracing`/`Logging`/`Idempotency` |
| Cqrs error | The dispatch-level error envelope | `CqrsError` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `Envelope<T>` | message carrier | `new` starts a fresh causal chain; `new_caused_by` inherits correlation + sets causation |
| `Command` / `Query` | trait (seam) | Write/read separation; `Command: validate_core::Validate` supertrait |
| `CommandBus` / `QueryBus` | trait | **Not object-safe** (`dispatch<C>` is generic) — hold the concrete bus or its `Arc` |
| `CqrsError` | error envelope | `Handler(e)` delegates `error_code`/`http_status`/`severity` to the original handler error |
| `IdempotencyStore` | trait (seam) | `mark_processed` runs only on `Ok` → failed handlers stay retryable |
| `CommandLayer`/`QueryLayer` | trait (seam) | Custom middleware; the final bus is a concrete decorated type |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The dispatch mechanism, the envelope + causal-chain model, the middleware-pipeline shape, and the bundled
  layers (`Tracing`/`Logging`/`Idempotency`). The only `dyn` is sealed inside `pub(crate)` erased bridges.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| The `Validate` trait | `validate-core` | `cqrs` depends *up* on the abstraction, not on `validation` |
| The validation middleware (`ValidationLayer`) | `validation` | It extends `cqrs` via `CommandLayer` |
| Identity injection into envelopes | `auth-context` (`cqrs-integration`) | It extends `cqrs`, not the reverse |
| Durable idempotency (Redis TTL) | the service | The in-memory store is a default to be swapped |

**The "do-not-depend-on" list:** never `tonic`/`rdkafka`/network/env — it stays a pure in-process bus.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | A command/query type has **exactly one** handler | builder `register` (eager) | `CqrsError::DuplicateRegistration` at build |
| I2 | Dispatching an unregistered type fails fast | `InMemoryCommandBus` dispatch | `CqrsError::HandlerNotFound` |
| I3 | No dynamic dispatch on the hot path (TypeId routing, native RPIT) | type system | `BoxFuture` only inside the sealed bridge |
| I4 | Idempotency marks only on `Ok` (failures stay retryable) | `IdempotencyLayer` | — |
| I5 | Queries carry no side-effects (no write path through `QueryBus`) | type system (write/read split) | — |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Build (startup).** `CommandBusBuilder::register::<C, _>(handler)` registers by `TypeId` (fails fast on
duplicates) → `.build()` yields an immutable `Arc`-backed `InMemoryCommandBus`. `MiddlewarePipeline::new(raw)`
then decorates it; the **first** `.layer()` is outermost.

**Dispatch (hot path).** A transport handler wraps the payload in `Envelope::new(correlation_id, payload)`
and calls `bus.dispatch(env)`. The decorated chain runs (e.g. Validation → Idempotency → Tracing → Logging →
`InMemoryCommandBus`): one `HashMap::get` (O(1), no lock) + one `Box::new` to cross the erasure boundary,
then `TypedHandlerBridge<H,C>` calls `Arc<H>.handle(envelope)`. Bus clone = `Arc::clone`.

**Causal chaining.** Inside a handler, `Envelope::new_caused_by(&incoming, payload)` inherits
`correlation_id` + metadata and sets `causation_id`, so a multi-step flow keeps one trace.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `validate-core` | upstream | Separated Interface | `Validate` supertrait on `Command` | every command's validate-ability |
| `error` | upstream | Conformist | `CqrsError: AppError`, handler `Error: AppError` | error delegation |
| every service | downstream | Published Contract | command/query buses | all application dispatch |
| `validation` | downstream | Open-Host (extends) | `ValidationLayer: CommandLayer` | input validation middleware |
| `auth-context` | downstream | Open-Host (extends) | `inject_into_envelope` (`cqrs-integration`) | identity propagation into envelopes |

> **Stability seam:** the `Command`/`Query`/`*Handler`/`*Bus` traits and `Envelope<T>` are public API; the
> non-object-safety of the dispatch traits is a contract callers must respect (hold concrete types).

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| `cqrs.command.dispatch` / `cqrs.query.dispatch` span | `tracing` (`TracingLayer`) | each dispatch (`otel.kind=INTERNAL`, `message.type/id`, `correlation.id`) | distributed-trace backends |
| start / complete-or-failed log | `tracing` (`LoggingLayer`) | each dispatch (`elapsed_ms`, `error.code`) | latency + error-rate dashboards |

Side-effect surface is the `IdempotencyStore` it writes to; the bundled in-memory store is process-local and
unbounded.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| TypeId routing, no reflection/vtable; `dyn` only in sealed `pub(crate)` bridges | [`README §Architecture`](../README.md) | Accepted |
| No `async_trait` — native RPIT end-to-end; `BoxFuture` only in erased bridges | [`README §Architecture`](../README.md) | Accepted |
| Idempotency marks only on `Ok` (failed handlers stay retryable) | [`README §Architecture`](../README.md) | Accepted |
| Write/read separation enforced by the type system | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — a CQRS bus; leverage is the zero-overhead hot path and one uniform pipeline
  shape across every service.
- **Stability:** stable contract — the traits and envelope are settled.
- **Volatility:** low — growth is new bundled layers, authored to the same engine invariants (no
  `async_trait`, no allocation on the common path).
- **Deferred capabilities:** a durable/TTL `IdempotencyStore` (Redis `SET NX EX`) — the seam exists; the
  default is in-memory.
