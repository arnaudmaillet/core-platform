# `resilience` — Domain & Functional Contract

> Egress fault tolerance: the pure Tower middleware that answers *"should this outbound call be attempted, retried, or fast-failed?"* — the client-side mirror of `traffic`.

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Cascading-failure protection at the outbound transport boundary (circuit breaker · retry · timeout) |
> | **Layer** | `foundation` — a pure Tower-middleware mechanism |
> | **Subdomain class** | **Generic** — standard resilience patterns; leverage is fleet-wide consistency + hot-reload |
> | **Primary abstraction(s)** | `ResilienceProfile` + the three `*Layer` types (`resilience::profile`, `::{circuit_breaker, retry, timeout}`) |
> | **Footprint** | pure (no IO, no `notify`, no spawned tasks); `serde` feature off by default |
> | **Failure posture** | **fail-fast** — an unhealthy downstream trips the circuit and requests are *not* forwarded (`CircuitOpen`) |
> | **Depends on** | `tower`, `arc-swap`, `tokio`, `thiserror`, `rand`, `error`, `serde` (optional) |
> | **Consumed by** | `transport` (gRPC/Kafka clients), the `cqrs` bus; configured via `infra-config` |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `resilience` is the fleet's authority for **egress fault tolerance**: it answers
**"is this downstream healthy enough to call, and if a call fails transiently, how do we retry without
amplifying the outage?"** — fleet-wide, without touching business logic.

**The hard problem.** Three patterns (timeout, circuit breaker, retry) must compose in a *load-bearing*
order and reconfigure live during an incident, while staying a pure, unit-testable library. `resilience`
keeps the mechanism pure (named profiles behind `ArcSwap` handles) and pushes parsing/validation/bindings
into `infra-config`, so a config push retunes a live channel with no rebuild.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Protect a *server* from inbound load (ingress) → that is `traffic`, the mirror crate.
- ❌ Parse or validate its config → owned by `infra-config`.
- ❌ Retry at the gRPC channel level → HTTP/2 bodies are streams; retry belongs at the application layer
  (see `transport`), the circuit/timeout layers wrap the channel.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Profile | A named class-of-service bundling one timeout + CB + retry | `ResilienceProfile`, `ResilienceProfileSpec` |
| Circuit breaker | The fail-fast state machine over a downstream's health | `CircuitBreakerLayer`, `CircuitBreakerConfig` |
| Retry policy | Whether an error at attempt N is retryable | `RetryPolicy`, `DefaultRetryPolicy` |
| Backoff | The delay schedule between attempts | `BackoffStrategy`, `ExponentialBackoff`, `JitterKind` |
| Resilience error | The middleware-emitted outcome envelope | `ResilienceError::{CircuitOpen, Timeout, MaxRetriesExhausted, Inner}` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `ResilienceProfile` | runtime handle | Bundles the three layers; timeout/CB behind shared `ArcSwap` for hot-reload |
| `ResilienceError<E>` | error envelope | `Inner(E)` is the *only* variant carrying downstream state; the rest are middleware-emitted |
| `CircuitBreakerLayer` / `…Service` | Tower layer | Owns the `Arc<StateMachine>`; clones share state (tonic clones per RPC) |
| `RetryLayer` | Tower layer | Requires `S: Clone` **and** `Req: Clone` (re-issues the request per attempt) |
| `BackoffStrategy` | trait (seam) | `next_delay(attempt)`; `ExponentialBackoff` defaults to `Full` jitter |

**Circuit-breaker state machine.**

```
Closed --(failure_threshold consecutive failures)--> Open
Open   --(open_duration elapsed)--> HalfOpen
HalfOpen --(success_threshold successes)--> Closed
HalfOpen --(any probe failure)--> Open          (half_open_max_calls caps concurrent probes)
```

> A request hitting an `Open` circuit is **not forwarded** — it returns `CircuitOpen` immediately.

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The three middleware layers, their state machines/config types, and the composition order. The circuit
  state, retry accounting, and timeout enforcement live here and nowhere else.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| TOML parsing / validation / bindings / file-watching | `infra-config` | Purity boundary — the mechanism links no `notify`/`toml` |
| Mapping `ResilienceError` → `TransportError`/`Status` | `transport` | Transport coupling stays out of the pure crate |
| The retryability *meaning* of an error | `error` (`AppError::is_retryable`) | `DefaultRetryPolicy` delegates to it |

**The "do-not-depend-on" list:** never `notify`, `toml`, `tonic`, or `http`. The `serde` feature (off by
default) is the only optional surface — with it off the crate links no serde/derive code.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Layer order is Timeout ⊃ CircuitBreaker ⊃ Retry | composition (caller) | circuit miscounts retries / trips too late |
| I2 | Config is sampled once per `call` (one `ArcSwap::load`) | each layer's `call` | inconsistent mid-decision values |
| I3 | A config swap never resets live state (counters, timers, circuit) | `ArcSwap` store | — |
| I4 | `Inner(E)` is the only variant carrying downstream error state | type system | — |
| I5 | Exponent is clamped (≤30) to avoid `u64` overflow in backoff | `ExponentialBackoff` | — |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Hot path — one `call`.** Outermost `TimeoutLayer` arms a deadline; `CircuitBreakerLayer` checks state
(`Open` → return `CircuitOpen` without forwarding) and, if `Closed`/`HalfOpen`, forwards; `RetryLayer`
re-issues on retryable errors with `ExponentialBackoff` + `Full` jitter up to `max_attempts`. Each layer
`ArcSwap::load`s its config once so the decision reasons against consistent values.

**Why the order is load-bearing.** The circuit must sit *outside* retry so it counts every attempt (incl.
retries) and trips on time; invert them and the circuit only ever sees the first call. Timeout outermost
bounds the *total* request budget including all retries.

**Reconfiguration.** `infra-config` calls `ResilienceProfile::apply(spec)`; the `ArcSwap` store is lock-free
and leaves circuit state, retry counters, and timers untouched — so a live channel retunes without a rebuild.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `error` | upstream | Conformist | `AppError::is_retryable` (`DefaultRetryPolicy`) | retry classification |
| `transport` | downstream | Published Contract | the three `*Layer`s + `ResilienceProfile` | every resilient gRPC/Kafka client |
| `cqrs` bus | downstream | Published Contract | wraps outbound dispatch | application-layer resilience |
| `infra-config` | downstream | Conformist (`serde`) | `ResilienceProfileSpec` | `[resilience]` parsing/hot-reload |
| `traffic` | sibling (mirror) | — | shares the catalog+bindings shape, opposite direction | symmetry |

> **Stability seam:** `ResilienceError`, the `*Layer` types, and `ResilienceProfile` are public API consumed
> by `transport`.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| circuit transition | `tracing` INFO (`prev`/`next`) | state machine moves | resilience dashboards |
| circuit tripped / probe failed | `tracing` WARN | threshold crossed / half-open probe fails | paging on `CircuitOpen` |
| retry scheduled / request timeout | `tracing` WARN (`attempt`, `delay_ms`, `timeout_ms`) | a retry is queued / deadline hit | warn-level alerts |

No OTel metric exports yet (a noted TODO); no external state mutation.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Pure middleware; profiles behind `ArcSwap`, parsing in `infra-config` | [`README §Architecture`](../README.md) | Accepted |
| Layer order Timeout ⊃ CircuitBreaker ⊃ Retry (circuit counts retries, trips on time) | [`README §Architecture`](../README.md) | Accepted |
| `JitterKind::Full` default to defeat fleet-wide thundering-herd retries | [`README §Architecture`](../README.md) | Accepted |
| No `RetryLayer` at the channel level (HTTP/2 body buffering) | [`transport README`](../../../platform/transport/README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — textbook resilience patterns; the leverage is fleet-wide uniformity + live
  retuning, not novelty.
- **Stability:** stable contract — production-ready, no stubs.
- **Volatility:** low — the three patterns are settled; growth is in backoff strategies / policies.
- **Deferred capabilities:** OTel metric instruments for the layers (today only `tracing` events).
