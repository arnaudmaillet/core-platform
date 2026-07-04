# `service-runtime` — Domain & Functional Contract

> The unified fleet bootstrap: implement one trait, get a deployable service. It answers *"what is the one boot sequence every service runs, and what does a service still own?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | The single boot sequence every service runs: telemetry → config + hot-reload → compose → serve (trace + traffic + health) → drain |
> | **Layer** | `platform` — the composition root shared by every `*-server` binary |
> | **Subdomain class** | **Supporting** — the operational backbone; one place to evolve fleet-wide process concerns |
> | **Primary abstraction(s)** | `Service` trait + `serve::<S>(addr)` (`service_runtime`) |
> | **Footprint** | IO/stateful — binds sockets, spawns the watcher + readiness + prune loops, owns shutdown |
> | **Failure posture** | **fail-closed at boot** (bad config never serves) + **dynamic health** (`NOT_SERVING` until probes pass) |
> | **Depends on** | `tonic`, `telemetry`, `infra-config`, `traffic`, `health`, `error`, `transport` |
> | **Consumed by** | every `crates/apps/<svc>-server` binary (via `serve::<S>(addr)`) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `service-runtime` is the fleet's authority for **the boot sequence**: it answers
**"how does every service start, observe, configure, rate-limit, report health, and drain identically — so a
service binary is a one-liner?"** The split is deliberate: the runtime owns process-wide concerns; the service
owns only its domain wiring, its concrete gRPC services, and its backend probes.

**The hard problem.** Process-wide concerns (observability, config IO + hot-reload, ingress rate-limiting,
socket binding, graceful shutdown, the readiness loop that drives gRPC health from backend liveness) are
identical across 17 services and easy to get subtly wrong. Centralising them behind one trait — with a
type-erased `RoutesBuilder` seam so the Tower layer stack never leaks into the service's signature — means a
new service is `~10` lines and a fleet-wide change is one edit here.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Own domain wiring (repos, caches, buses, workers) → the service's `build`.
- ❌ Define the telemetry/config/traffic/health *mechanisms* → those are `telemetry`/`infra-config`/`traffic`/`health`.
- ❌ Expose the Tower layer types to services → the `register` seam hides them.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Service | The trait a deployable service implements (the only surface) | `Service` |
| Composition root | The service's pure graph-building step | `Service::build` |
| Register | Plugging the service's concrete gRPC services onto a type-erased builder | `Service::register`, `RoutesBuilder` |
| Readiness loop | The background poll mapping probes → gRPC health status | `spawn_readiness` |
| Traffic prune loop | The background loop bounding rate-limiter memory | `spawn_traffic_prune` |
| Telemetry control sink | The bridge applying `[telemetry]` config to the live pipeline | `TelemetryControlSink` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `Service` | trait (seam) | `NAME`/`VERSION`/`GRPC_SERVICE_NAME` consts + `build`/`health_probes`/`register` |
| `serve::<S>(addr)` | entrypoint | The entire production boot+serve+drain; a binary is just this call |
| `GRPC_SERVICE_NAME` | const contract | **Must** equal the concrete server's `NamedService::NAME` (the health key) |
| re-exports | ergonomics | `HealthProbe`/`FnProbe` (from `health`), `InfraRegistry` (from `infra-config`) |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**

| Concern | Owner |
|---|---|
| Telemetry init, OTLP, log/sampling dials | **runtime** |
| Config load + hot-reload watcher + telemetry sink bridge | **runtime** |
| Ingress trace + rate-limit layers, prune loop | **runtime** |
| gRPC health, readiness loop, graceful shutdown | **runtime** |

**The service owns** (not this crate): domain wiring (`build`), concrete gRPC services + reflection
(`register`), backend probes (`health_probes`).

**The "do-not-depend-on" list:** it composes the platform/foundation crates but owns none of their mechanisms;
it must not pull in a service/domain crate. The `TelemetryControlSink` bridge lives here precisely because it
needs both `infra-config` and `telemetry`, which must not depend on each other.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Boot is fail-closed: a malformed config stops the pod from ever serving | `serve` (config load) | pod never becomes ready |
| I2 | `GRPC_SERVICE_NAME` must equal the concrete `NamedService::NAME` | contract convention | client sees `NOT_SERVING` forever |
| I3 | With probes, a service is `NOT_SERVING` until all pass; any failure demotes it | `spawn_readiness` | readiness reflects real backend reachability |
| I4 | The config watcher guard outlives the process | `serve` keeps `_watcher` in scope | config freezes at boot value |
| I5 | Tower layer types never reach `register` | type-erased `RoutesBuilder` seam | leaked layer types in service signatures |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**`serve::<S>(addr)` — the one boot sequence.**

1. `telemetry::init` (logs + OTLP traces + metrics); the guard is kept (drop flushes spans/logs). *(boot)*
2. `load_from_path` + `InfraRegistry::from_config` — **fail-closed**; a bad document aborts the boot. *(boot)*
3. Register the `TelemetryControlSink` so `[telemetry]` dials apply immediately and on every later change. *(boot)*
4. `spawn_watcher` (kept alive) — hot-reload of resilience/cache/traffic/telemetry. *(background)*
5. `S::build(infra)` — the service composition root. *(boot)*
6. Build the gRPC server: `InboundTraceLayer` (outer) + `TrafficLayer` (inner, only if `[traffic]` present);
   add the health service + `S::register(routes)`. *(boot)*
7. `spawn_readiness` (probes → gRPC health, transition-only writes) + `spawn_traffic_prune` (bounds limiter
   memory). *(background)*
8. `serve_with_shutdown` — serve until SIGTERM/SIGINT, then drain in-flight requests. *(lifetime → shutdown)*

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `telemetry` | upstream | Conformist | `init` + `TelemetryControl` | observability boot + live dials |
| `infra-config` | upstream | Conformist | `load_from_path`/`spawn_watcher`/`InfraRegistry` | config boot + hot-reload |
| `transport` | upstream | Conformist | `GrpcServerBuilder` (+ traffic) | the gRPC server stack |
| `health` | upstream | Conformist | `HealthProbe` (re-exported) | the readiness loop |
| every `*-server` binary | downstream | Published Contract | `impl Service` + `serve::<S>` | the entire fleet's boot |

> **Stability seam:** the `Service` trait (esp. `GRPC_SERVICE_NAME` ↔ `NamedService::NAME`) is the single
> surface every service binds to; changing it touches all 17.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| `gRPC server listening` / `shutdown complete` | `tracing` INFO | boot / drain | ops |
| `gRPC health status changed` | `tracing` INFO | a readiness transition | K8s readiness probes |
| `traffic registry pruned` | `tracing` DEBUG | each prune tick | limiter-memory monitoring |

Side effects: binds the listen socket, spawns the watcher/readiness/prune tasks, installs the SIGTERM + SIGINT handlers.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| One `Service` trait owns the boot sequence; a binary is a one-liner | [`README §Architecture`](../README.md) | Accepted |
| Type-erased `RoutesBuilder` seam keeps Tower layers out of service signatures | [`README §Architecture`](../README.md) | Accepted |
| Dynamic gRPC health driven by backend probes (not pinned `SERVING` at boot) | [`README §Architecture`](../README.md) | Accepted |
| Fail-closed config boot + single-writer hot-reload | [`infra-config README`](../../../foundation/infra-config/README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Supporting — the operational backbone; leverage is fleet-wide uniformity of process
  concerns.
- **Stability:** stable contract — the `Service` trait is settled across 17 services.
- **Volatility:** low — new process-wide concerns (a new background loop, a new layer) are added here once.
- **Deferred capabilities:** richer drain/health hooks for stateful edge services (noted in the realtime
  work); SIGTERM is now handled alongside SIGINT.
