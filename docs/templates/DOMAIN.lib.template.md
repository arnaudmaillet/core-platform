<!--
================================================================================
 SHARED-LAYER DOMAIN DOC — LIBRARY BLUEPRINT (foundation/ + platform/)
================================================================================
 Copy this file to crates/<layer>/<crate>/docs/DOMAIN.md and fill every
 <placeholder>. Mirror to DOMAIN.fr.md (English is canonical; the i18n drift
 gate enforces hash parity on every DOMAIN.*.md).

 WHY A SEPARATE TEMPLATE.
   The service template (DOMAIN.template.md) documents a *bounded context* — a
   System of Record/Reference with aggregates, Kafka events, and data ownership.
   The crates in `foundation/` and `platform/` are NOT bounded contexts: they own
   no business data, emit no domain events, and have no aggregate. They own a
   *technical capability* (a mechanism, a contract, a boot sequence) consumed by
   the contexts above them. This template keeps the same 10-section skeleton and
   the same CORE/DEEP tiering so the catalog stays uniform and grep-able, but
   reframes each section through a library lens.

 SCOPE BOUNDARY — read before writing:
   This file explains the ARCHITECTURAL CONTRACT: the capability owned, the
   invariants enforced, the dependency direction, and the coupling to consumers.
   It must NOT restate the README (public API signatures, env vars, build,
   runbook, gotchas). If a line tells a reader HOW to call/run the crate, it
   belongs in README.md. If it tells them WHY the boundary is where it is, it
   belongs here.

 GROUND TRUTH:
   Derive every statement from the code — `src/lib.rs`, the public types, the
   Cargo.toml dependency edges, and the feature flags. Never invent an ADR
   number: foundation/platform crates have no dedicated ADRs (ADR-0001..0017 are
   service decisions); their rationale lives in the README §Architecture and is
   linked, not fabricated.

 TIER RULE (how much to fill)
   CORE — every crate:
     Domain Card · Technical Capability & Non-Goals · Ubiquitous Language
     · Public Model & Contract Surface · Ownership & Architectural Boundaries
     · Invariants & Contract Rules
   DEEP — full prose for crates with real machinery (a state machine, a boot
   sequence, a hot-reload path, a consumer runtime):
     Control Flow & Lifecycle · Crate Coupling · Emitted Signals & Side-Effects
     · Decisions & Rationale · Classification & Evolution
   Do NOT delete DEEP sections on a thin leaf crate — collapse each to one honest
   line, e.g. "N/A — pure contract crate, no runtime control flow", so the
   catalog stays uniform.

 VOICE
   An architectural contract, not an essay. Prefer tables. Every sentence must
   constrain a caller, a maintainer, or a reviewer — if it constrains none, cut it.
================================================================================
-->

# `<crate>` — Domain & Functional Contract

<!-- Tagline rule: name the technical capability and the one question it answers
     for the fleet, not a feature list. -->

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | `<the one technical concern this crate owns>` |
> | **Layer** | `<foundation \| platform>` — `<one-line: where it sits in the dependency graph>` |
> | **Subdomain class** | `<Generic \| Supporting>` — `<why it's classed so (technical, not product, value)>` |
> | **Primary abstraction(s)** | `<Trait/Type>` (`<crate::path>`) |
> | **Footprint** | `<pure (no IO/no spawn) \| IO/stateful \| dev-only>` — `<feature-gated surface, if any>` |
> | **Failure posture** | `<fail-closed \| fail-open \| N/A>` — `<one-line consequence>` |
> | **Depends on** | `<workspace crates it links>` |
> | **Consumed by** | `<workspace crates / layer that link it>` |
> | **Decision log** | `<ADR link if one exists, else: none — rationale in README §Architecture>` |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `<crate>` is the fleet's authority for **<the one technical capability>**: it answers
**"<the single question the rest of the fleet delegates to it>"**.

**The hard problem** (1–2 sentences): `<the cross-cutting tension that forced a dedicated shared crate
rather than per-service code>`.

**Non-goals — what this crate deliberately does NOT do** (the boundary, stated as denials):
- ❌ `<concern that looks adjacent but belongs to crate X>` → owned by `<crate>`.
- ❌ `<…>`

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

> Technical terms used *by this crate's public surface*. The code symbol is mandatory — a term with
> no symbol is aspirational, not real.

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| `<Term>` | `<precise definition>` | `<path::Type>` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

> The library equivalent of a domain model: the public types/traits and the invariant each one
> guards. Name the type, not its full signature (that's the README's job).

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `<Trait>` | trait (seam) | `<the contract implementors must honour>` |
| `<Type>` | value type | `<validity rule enforced at construction>` |

**Lifecycle / state machine** (only if the crate owns one):

```
<State> --(<transition>)--> <State> --> <Terminal>
```

> `<the rule that makes illegal transitions unreachable, and the error/Status that rejects them>`.

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

> The most important section for a shared crate. The "boundary" here is a *dependency-direction*
> rule, not a data-ownership rule — violating it (e.g. adding `tonic` to a pure crate) is an
> architecture bug, not a doc nit.

**This crate owns:**
- `<the mechanism/contract>` — `<what part of the concern lives here and nowhere else>`.

**This crate deliberately does NOT own / must NOT link** (the purity / inversion rule):

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| `<IO / parsing / transport / policy>` | `<crate>` | `<the layering reason>` |

**The "do-not-depend-on" list:** `<dependencies this crate must never grow, and the architectural
guarantee that forbids them>`.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

> Rules that must hold for every consumer in every reachable state. State the rule, then the layer
> that enforces it (type system / construction / runtime) — an unplaced invariant is unenforced.

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `<rule that must always hold>` | `<type system / ctor / runtime check>` | `<error code / panic / compile error>` |

<!-- Where a test proves an invariant, name it: "I1 — proven by tests::rejects_x". -->

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

> How the mechanism runs at runtime: the boot sequence, the hot path, the background loop, the
> hot-reload swap, or the per-message state machine. Distinguish the **hot path** (per-call, must be
> cheap) from **background / boot** work. Collapse to one line for a pure contract crate.

**`<flow name>`**

1. `<trigger>` → `<step>` (`<hot path \| boot \| background>`)
2. …

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

> This crate's edges in the workspace dependency graph, stated with the coupling pattern so the
> direction is explicit. Upstream = crates it links; downstream = crates that link it.

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| `<crate>` | upstream (we depend) | `<Separated Interface \| Conformist \| ACL>` | `<trait / type>` | `<impact on us>` |
| `<crate>` | downstream (depends on us) | `<Published Contract \| OHS>` | `<our trait / type>` | `<impact on them>` |

> **Stability seam:** `<the type/trait that is public API and whose change is a breaking change>`.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

> A shared crate emits no *domain* events. State what it DOES emit: `tracing` events, metrics, Kafka
> envelopes it relays (not owns), or "none — pure". Answers "what observable trace does using this
> crate leave?".

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| `<event / metric>` | `<tracing \| metric \| relayed Kafka>` | `<trigger>` | `<dashboards / alerts>` |

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

> Why the boundary is where it is. Foundation/platform crates have no dedicated ADRs — link the
> README §Architecture (the authoritative rationale) and any cross-cutting memory; never fabricate
> an ADR number.

| Decision | Where recorded | Status |
|---|---|---|
| `<the locked architectural choice>` | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** `<Generic \| Supporting>` — `<investment implication: commodity vs leverage>`.
- **Stability:** `<stable contract \| evolving>` — `<what a breaking change here would cost the fleet>`.
- **Volatility:** `<how often the surface changes, and why>`.
- **Deferred capabilities:** `<modeled-for but not built; the seam left for them>`.
