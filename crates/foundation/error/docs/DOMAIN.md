# `error` — Domain & Functional Contract

> The distributed-error contract: one trait, one wire shape, zero leakage — it answers *"how does every service expose its own errors uniformly, observably, and without leaking internals to clients?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | The workspace-wide error contract: a trait, a severity vocabulary, a typed envelope, and a client-safe wire shape |
> | **Layer** | `foundation` — a near-root contract crate (almost everything depends on it) |
> | **Subdomain class** | **Generic** — a cross-cutting contract; its leverage is uniformity + leak-safety |
> | **Primary abstraction(s)** | `AppError` + `DistributedError<E>` (`error::traits`, `error::context`) |
> | **Footprint** | pure (no IO, no state, no background threads); `axum` is a dev-dependency only |
> | **Failure posture** | N/A — it *describes* failures; it can never *cause* one (no IO, no state) |
> | **Depends on** | `thiserror`, `tracing`, `http`, `uuid`, `chrono` |
> | **Consumed by** | `cqrs`, `transport`, `resilience`, `validation`, and every service crate |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `error` is the fleet's authority for the **error contract**: it answers
**"how does a service expose its own typed error enum so that logging, paging, retry classification, and
the client-facing JSON are uniform across every service — and trace identifiers never reach a client?"**

**The hard problem.** Every service needs its *own* error enum (to pattern-match on), yet the platform
needs *one* observable, client-safe output. `error` reconciles this with a typed envelope
(`DistributedError<E>` keeps the concrete type — no `Box<dyn Error>`) and a strict disclosure boundary
(`trace_id`/`span_id` live on the context for logs but are structurally absent from the client wire shape).

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Define any domain/business error → each service owns its enum; this crate owns only the contract.
- ❌ Decide retry behaviour → it exposes `is_retryable()`; `resilience` acts on it.
- ❌ Own logging configuration (format, sampling, alerting) → that is the consuming binary's bootstrap.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| App error | The contract a service implements on its enum | `AppError` |
| Error code | Stable, client-visible, machine-readable code (e.g. `AUTH_TOKEN_EXPIRED`) | `AppError::error_code` |
| Severity | The unified urgency vocabulary driving paging/log level | `Severity` |
| Context | Request/trace metadata wrapping an error | `ErrorContext` |
| Distributed error | The typed, type-preserving envelope | `DistributedError<E>` |
| Api error response | The framework-agnostic, leak-free client JSON | `ApiErrorResponse`, `into_api_response` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `AppError` | trait (seam) | Only `error_code()` + `http_status()` are required; the rest carry production-safe defaults |
| `IntoApiResponse` | blanket trait | One canonical `to_api_response` for every `AppError`; must **not** be overridden |
| `DistributedError<E>` | typed envelope | Preserves the concrete `E` end-to-end (no erasure); `.log()` emits trace+span ids |
| `ErrorContext` | value type | Carries `trace_id`/`span_id` — present in logs, **absent** from the client wire shape |
| `ApiErrorResponse` | wire shape | The only struct sent to clients; contains no trace/span ids |
| `Severity` | enum | `Ord` as `Critical < High < Medium < Low < Info` ("higher urgency = lower value") |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The four pillars — contract (`AppError`/`IntoApiResponse`), vocabulary (`Severity`), context
  (`ErrorContext`/`DistributedError`), and wire format (`ApiErrorResponse`). The leak boundary is enforced
  *structurally* here.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| Any concrete domain error enum | each service crate | This crate is contract-only, consumer-agnostic |
| Logging format / sampling / alert routing | the consuming binary (`telemetry`) | Operational policy is not the contract's job |
| `axum`/HTTP framework glue | the service (newtype) | `axum` stays a dev-dependency; the wire shape is framework-agnostic |

**The "do-not-depend-on" list:** never a service crate, never `axum`/`tonic` in non-dev deps, never network
IO or state — so `error` can never be the cause of a cascading failure.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `error_code` is stable public API (clients/dashboards key off it) | contract convention | a rename is a breaking change requiring migration |
| I2 | `trace_id`/`span_id` never reach a client | type system (`ApiErrorResponse` omits them) + `into_api_response` strips them | structural leak only if you serialize `ErrorContext` directly |
| I3 | The concrete error type is preserved end-to-end (no `Box<dyn Error>`) | `DistributedError<E>` generic | loss of pattern-matching |
| I4 | New `AppError` methods must carry a production-safe default | trait definition | breaks existing implementors |
| I5 | The `IntoApiResponse` blanket impl is not overridden | blanket impl | divergent client shapes |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Error path.** A service's enum implements `AppError`; the boundary wraps it as `DistributedError<E>` with
an `ErrorContext`. `.log()` emits one `tracing` event at `severity().log_level()` (with `trace_id`/`span_id`
*inside* the log). The client body is built through `into_api_response(&err)` — which strips trace/span ids —
and returned with `http_status()`. No heap on the hot path: `AppError` methods return `&'static str` and the
envelope is stack-allocated until returned (services may `Box` it on latency-sensitive paths to keep
`Result` small).

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| every service crate | downstream | Published Contract | `impl AppError` on their enum | uniform error output fleet-wide |
| `cqrs` | downstream | Conformist | `CqrsError` implements/delegates `AppError` | bus error mapping |
| `resilience` | downstream | Conformist | `AppError::is_retryable` | retry classification |
| `validation` | downstream | Conformist | `ValidationError: AppError` (422) | validation error mapping |
| `transport` | downstream | Conformist | `grpc_severity` / error mapping | gRPC error severity |

> **Stability seam:** `AppError` (esp. `error_code`) and `ApiErrorResponse` are the contract every consumer
> binds to; `error_code` changes are client-breaking.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| structured error log | `tracing` (level = severity) | `DistributedError::log()` is called | log pipeline; correlate by `request_id` (client-visible) or `trace_id`+`span_id` (logs only) |

No metrics, no external store. The only side effect is the optional `tracing` event.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Typed envelope (`DistributedError<E>`), no `Box<dyn Error>` erasure | [`README §Architecture`](../README.md) | Accepted |
| Two-tier disclosure — only `error_code`+`http_status` required, rest defaulted | [`README §Architecture`](../README.md) | Accepted |
| Leakage made structurally impossible (trace/span absent from the wire shape) | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — a cross-cutting contract; leverage is uniformity, observability, and
  leak-safety, not business value.
- **Stability:** stable contract — `error_code` is treated as public API.
- **Volatility:** low — new `AppError` methods are additive (defaulted); the wire shape is settled.
- **Deferred capabilities:** none structural; richer details payloads serialize through
  `ApiErrorResponse.details` as needed.
