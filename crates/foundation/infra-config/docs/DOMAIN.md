# `infra-config` — Domain & Functional Contract

> Externalized configuration & fail-closed hot-reload: the IO/policy layer that answers *"what numbers do the pure middleware crates run with, and how do they change without a restart?"*

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Externalized infra config — parse, validate, resolve bindings, and hot-reload the `[section]`s the pure crates must not parse themselves |
> | **Layer** | `foundation` — the policy/IO layer feeding the pure middleware crates |
> | **Subdomain class** | **Supporting** — the operational control plane for fleet-wide policy; high leverage during incidents |
> | **Primary abstraction(s)** | `InfraRegistry` + `Reloadable` (`infra_config::infra`, `infra_config::reload`) |
> | **Footprint** | IO/stateful — owns file IO, TOML parsing, a `notify` watcher, and `ArcSwap` swaps |
> | **Failure posture** | **fail-closed** — a malformed or invalid document is rejected; the previous good config stays live |
> | **Depends on** | `notify`, `toml`, `serde`, `arc-swap`, `tokio`, `resilience` + `traffic` (`serde`) |
> | **Consumed by** | `service-runtime` (loads + watches); services read resolved `[cache]`/`[resilience]` profiles |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `infra-config` is the fleet's authority for **externalized infrastructure policy**: it
answers **"where do the timeouts/quotas/TTLs/sampling come from, and how do they retune live?"** — so the
pure mechanism crates (`resilience`, `traffic`, the cache adapters, telemetry dials) stay free of IO.

**The hard problem.** Incident-critical knobs (a circuit timeout, a rate quota, a log filter) must change
*without a redeploy*, but the crates that *use* them must remain pure and unit-testable. `infra-config`
absorbs all the dangerous parts — file IO, parsing, validation, K8s ConfigMap inode-swap semantics,
race-free swapping — behind one fail-closed reload path shared by every section.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Own the *mechanism* a section configures (Tower layers, the limiter, cache adapters) → those are the
  pure crates (`resilience`, `traffic`, …).
- ❌ Apply telemetry dials directly → it exposes a `TelemetrySink`; `service-runtime` bridges it to `telemetry`.
- ❌ Hot-reload *topology* (which profiles/sections exist, which dependency binds where) → fixed at boot.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Section | One infrastructure category in the document | `[resilience]`, `[cache]`, `[traffic]`, `[telemetry]` |
| Catalog | Named profiles + a binding table, one shape per section | `Catalog<L>`, `catalog::validate_bindings` |
| Binding | A dependency/namespace name → a class-of-service profile | (resolved inside each `*Registry`) |
| Wire vs Runtime | Flat serde spec parsed from TOML vs `ArcSwap`-backed handle the data path reads | `*ProfileSpec` vs `*Profile` |
| Registry | The resolved, hot-reloadable holder for a section | `InfraRegistry`, `ResilienceRegistry`, `CacheRegistry`, `TrafficRegistry`, `TelemetryRegistry` |
| Reloadable | The watcher's target — parse + validate + swap | `Reloadable::reload` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `InfrastructureConfig` | parsed document | `from_toml` + `validate`; new sections are `Option<…>` for backward compat |
| `InfraRegistry` | aggregate registry | Resolves every section; `apply` swaps all-or-nothing |
| `Reloadable` | trait (seam) | Decouples the watcher from any section's shape; `reload(raw)` is fail-closed |
| `Catalog<L>` | shared shape | One resolution/validation path reused by every section |
| `spawn_watcher` | function | Returns a guard that **must stay alive** for the watch to continue |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- File IO, TOML parsing, fail-closed validation, fleet bindings, and the `notify`-based hot-reload path —
  the *policy plumbing* every pure mechanism crate must stay free of.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| Tower layers / circuit-breaker state | `resilience` | The mechanism is pure; this crate only supplies its numbers |
| The GCRA limiter | `traffic` | Same purity split |
| The telemetry pipeline | `telemetry` | This crate exposes a `TelemetrySink`; the bridge lives in `service-runtime` |

**The "do-not-depend-on" list:** never `tonic`/`http` or any service crate. It depends *up* on the pure
crates (`resilience`, `traffic`) only for their `serde` wire types — never their runtime.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Every present section validates *before* any section swaps (all-or-nothing) | `apply` / `Reloadable::reload` | `ConfigError::Validation`; previous config stays live |
| I2 | All swaps happen in **one** writer task (no torn reads/races) | the single spawned watcher | — |
| I3 | A binding/`default_profile` must reference a defined profile | `catalog::validate_bindings` | `ConfigError::Validation` |
| I4 | The watcher observes the **parent directory**, not the file path | `spawn_watcher` | (else K8s ConfigMap inode-swap goes undetected) |
| I5 | Topology (sections/profiles/bindings) is fixed at boot; only *contents* hot-reload | resolve-time wiring | requires restart |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Boot.** `load_from_path` reads + parses `infrastructure.toml`; `InfraRegistry::from_config` validates
every section and resolves bindings into `ArcSwap`-backed runtime handles. A malformed/invalid document
fails the boot — the pod never serves bad config.

**Hot-reload loop.** `spawn_watcher` watches the *parent directory* (K8s swaps the `..data` symlink inode,
so a file-path watch goes deaf after the first change). A `notify` event → coalesce bursts → re-read →
`Reloadable::reload`: parse + validate **all** present sections, then swap **all** via `ArcSwap` (fail-closed,
all-or-nothing). The guard returned by `spawn_watcher` must outlive the process.

**Data path.** Consumers hold runtime handles (`*Profile`) and `ArcSwap::load` a snapshot per operation —
lock-free, always consistent within a single decision.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `resilience` | upstream | Conformist (`serde` types) | `ResilienceProfileSpec` | `[resilience]` parsing |
| `traffic` | upstream | Conformist (`serde` types) | `TrafficProfileSpec` | `[traffic]` parsing |
| `service-runtime` | downstream | Published Contract | `load_from_path`, `spawn_watcher`, `InfraRegistry` | fleet boot + hot-reload |
| `telemetry` | indirect | Separated Interface | `TelemetrySink` (bridged by `service-runtime`) | live log/sampling retuning |

> **Stability seam:** `InfraRegistry`, `Reloadable`, and the `ConfigError` variants are the public contract
> `service-runtime` builds on.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| reload applied / rejected | `tracing` | a config document is swapped or fails validation | ops dashboards during a config push |
| filesystem watch | side-effect | a `notify` watcher on the config's parent dir | the OS inotify/FSEvents layer |

It mutates no external store; its only side effects are reading the config file and swapping in-process
`ArcSwap` handles.

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| Pure mechanism vs IO/policy split (middleware links no `notify`/`toml`) | [`README §Architecture`](../README.md) | Accepted |
| Fail-closed, all-sections-or-nothing swap in a single writer task | [`README §Architecture`](../README.md) | Accepted |
| Watch the parent directory to survive K8s ConfigMap inode swaps | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Supporting — the control plane for fleet-wide policy; its leverage is operational
  (retune an incident without a redeploy).
- **Stability:** evolving — adding a new `[section]` is the expected growth path (a spec + a live type + a
  registry, reusing `Catalog<L>`).
- **Volatility:** medium — section *contents* are config; the *resolution machinery* is settled.
- **Deferred capabilities:** none structural; each new infra concern becomes a new section.
