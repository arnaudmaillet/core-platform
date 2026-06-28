# `health` — Domain & Functional Contract

> A graph-leaf probe contract: the abstraction that answers *"is this backend reachable right now?"* — so storage crates can publish probes the runtime polls, with no edge between them.

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | The liveness/readiness probe contract that drives a service's gRPC health status |
> | **Layer** | `foundation` — a deliberate graph-leaf (tiny, depends on `async-trait` + `anyhow` only) |
> | **Subdomain class** | **Generic** — a one-trait abstraction; its value is the decoupling, not the code |
> | **Primary abstraction(s)** | `HealthProbe` + `FnProbe` (`health`) |
> | **Footprint** | pure (no IO, no spawn) — it defines the contract, never opens a connection |
> | **Failure posture** | **fail-to-`NOT_SERVING`, self-clearing** — any `Err` demotes the service until a later tick clears it |
> | **Depends on** | `async-trait`, `anyhow` |
> | **Consumed by** | `service-runtime` (polls probes → gRPC health), storage crates (`scylla`/`redis`/`postgres` expose probes) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `health` defines the **probe contract** the fleet uses to gate readiness: it answers
**"can the runtime ask any backend 'are you reachable?' without depending on that backend's crate?"**

**The hard problem.** A storage crate knows *how* to check its backend; the runtime knows *when* to check
and *what* to do with the result. Putting the trait in either side would couple storage to the runtime (or
vice-versa). A tiny leaf crate lets both sides depend only on it, so a storage crate publishes a probe and
the runtime consumes it with **no edge between them** — the entire reason this crate exists.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Open connections or run real queries → the storage crate's `health::probe` does that.
- ❌ Schedule polling or know about gRPC → owned by `service-runtime`'s readiness loop.
- ❌ Hold sticky failure state → there is none; readiness re-derives from the latest tick.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Probe | A cheap reachability check the runtime polls each tick | `HealthProbe` |
| Name | Short identifier for logs (`"scylla"`, `"redis"`) | `HealthProbe::name` |
| Check | The async reachability ping; `Ok` = reachable | `HealthProbe::check` |
| Fn-probe | A closure-backed probe for a bespoke dependency | `FnProbe` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `HealthProbe` | trait (seam) | `check()` must be cheap + idempotent (runs every readiness tick); `Ok(())` = reachable, any `Err` demotes |
| `FnProbe<F>` | adapter | Wraps an `Fn() -> Future` so a probe needs no bespoke type; the closure is re-invoked every tick |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- The probe *contract* only — the trait shape and the closure adapter. Nothing else.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| Live backend clients + the actual check | storage crates (`scylla`/`redis`/`postgres`) | They own their client; they depend only on this leaf |
| Polling cadence + gRPC health wiring | `service-runtime` | The runtime schedules and maps results to `ServingStatus` |

**The "do-not-depend-on" list:** never a storage client, never `tonic`, never `service-runtime`. Adding any
such edge would reintroduce the coupling this crate was created to break.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `check()` is cheap (a reachability ping, not a query) | contract convention | a heavy probe flaps the service under load |
| I2 | Any single probe `Err` demotes the whole service | `service-runtime` readiness loop | service → `NOT_SERVING` until next clean tick |
| I3 | No sticky failure state — readiness re-derives each tick | by design (no state here) | — |
| I4 | A `FnProbe` closure must be re-callable (`Fn`, not `FnOnce`) | type bound | compile error |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

N/A — pure contract crate, no runtime control flow of its own. The polling loop (first tick immediate,
transition-only writes to the gRPC reporter) lives in `service-runtime`; the actual reachability check lives
in each storage crate's `health::probe`. This crate is only the trait that joins them.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| storage crates | downstream | Published Contract | `impl HealthProbe` over their client | every service's readiness signal |
| `service-runtime` | downstream | Published Contract | polls `Vec<Arc<dyn HealthProbe>>` | the readiness → gRPC-health mapping |

> **Stability seam:** the `HealthProbe` trait is the entire public API; a signature change ripples to every
> storage crate *and* the runtime simultaneously — treat it as a hard breaking change.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

N/A — pure. It emits nothing; the readiness `tracing` events (`"health status changed"`) are emitted by
`service-runtime` when it acts on a probe result.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Place the probe trait in a graph-leaf so storage and runtime never couple | [`README §Architecture`](../README.md) | Accepted |
| Fail-to-`NOT_SERVING`, self-clearing (no sticky state) | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — a commodity contract; its leverage is purely the dependency-graph shape.
- **Stability:** stable contract — the trait has settled; changing it is a fleet-wide breaking change.
- **Volatility:** very low — the surface is two items.
- **Deferred capabilities:** none; richer health (degraded vs down, weighted dependencies) would be a new
  enum on the result, but is not modelled today.
