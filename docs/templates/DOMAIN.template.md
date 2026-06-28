<!--
================================================================================
 FLEET SERVICE DOMAIN DOC — STANDARD BLUEPRINT
================================================================================
 Copy this file to crates/services/<svc>/docs/DOMAIN.md and fill every
 <placeholder>. Mirror to DOMAIN.fr.md (English is canonical; the i18n drift
 gate enforces hash parity once extended to DOMAIN.*.md).

 SCOPE BOUNDARY — read before writing:
   This file explains MEANING (what business capability, what invariants, what
   it owns). It must NOT restate the README (ports, env, build, runbook) or the
   proto (wire/enum schema). If a line tells a reader how to RUN something, it
   belongs in README.md. If it tells them WHY a rule exists, link the ADR.

 GROUND TRUTH:
   Derive every statement from the code — the crate, its `*.v1` protos, and the
   event-topology registry guard. Do NOT cite the quarantined legacy C4 in
   docs/_legacy/. C4 references in §6/§7 are commented out until a corrected C4
   model is regenerated from these docs.

 TIER RULE (how much to fill)
   CORE — every service, every tier:
     Domain Card · Business Capability & Non-Goals · Ubiquitous Language
     · Domain Model · Data Ownership & Boundaries · Invariants & Business Rules
   DEEP — full prose for TIER-0 / TIER-1:
     Workflows & Orchestration · Context Relationships · Domain Events
     · Decisions & Rationale · Subdomain Classification & Evolution
   Do NOT delete DEEP sections on a TIER-2 service — collapse each to one honest
   line, e.g. "N/A — derived read-model, no cross-context orchestration", so the
   domain catalog stays uniform and grep-able.

 VOICE
   This is a domain contract, not an essay. Prefer tables over prose. Every
   sentence must constrain a modeler, a caller, or a reviewer — if it constrains
   none of them, cut it.
================================================================================
-->

# `<svc>` — Domain & Functional Contract

<!-- Tagline rule: name the bounded context and the one question it answers, not a feature list. -->

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | `<the context name in the ubiquitous language>` |
> | **Subdomain class** | `<Core \| Supporting \| Generic>` — `<one-line why it's classed so>` |
> | **System of …** | `<Record (SoR) \| Reference (SoRef) \| Connection \| Evidence>` for `<the data>` |
> | **Aggregate root(s)** | `<Aggregate>` (`<crate::domain path>`) |
> | **Tier** | `<TIER-0 \| TIER-1 \| TIER-2>` |
> | **Failure posture** | `<fail-closed \| fail-open>` — `<one-line consequence>` |
> | **Upstream contexts** | `<ctx>` via `<pattern: Conformist \| ACL \| Customer>` |
> | **Downstream contexts** | `<ctx>` via `<Published Language topic.* \| OHS gRPC>` |
> | **Decision log** | [`ADR-00NN`](../../../../docs/adr/00NN-*.md), … |

---

## 1. Business Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `<svc>` is the authority for **<the one business capability>**: it answers
**"<the single domain question it exists to answer>"**.

**The hard problem** (1–2 sentences): `<the domain tension that forced a dedicated context>`.

**Non-goals — what this context deliberately does NOT do** (the boundary, stated as denials):
- ❌ `<capability that looks adjacent but belongs to ctx X>` → owned by `<ctx>`.
- ❌ `<…>`

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

> Terms used *inside this context only*. Cross-context terms live in
> `docs/domain/UBIQUITOUS_LANGUAGE.md`. The code symbol is mandatory — a term with no symbol is
> aspirational, not ubiquitous.

| Term | Meaning in this context | Code symbol |
|---|---|---|
| `<Term>` | `<precise definition; note where it differs from the same word elsewhere>` | `<domain::Type>` |

---

## 3. Domain Model &nbsp;·&nbsp; CORE

**Aggregates, entities, value objects** (and the consistency boundary each enforces):

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `<Aggregate>` | aggregate root | `<the transaction/consistency boundary>` |
| `<Entity>` | entity | `<identity rule>` |
| `<VO>` | value object | `<validity rule enforced at construction>` |

**Lifecycle / state machine** (if the aggregate has one):

```
<State> --(<command/event>)--> <State> --(...)--> <Terminal>
```

> **Legal transitions only.** `<list illegal transitions and the error code rejecting them, e.g. CHT-2003>`.

---

## 4. Data Ownership & Boundaries &nbsp;·&nbsp; CORE

> The single most important section. A boundary violation here is a distributed-systems bug, not
> a doc nit.

**This context is the source of truth for:**
- `<data>` — `<store, e.g. ScyllaDB keyspace `x`>`. No other service writes this.

**This context holds copies it does NOT own (read-model / denormalization):**

| Copied data | Owned by | Kept fresh via | Staleness tolerance |
|---|---|---|---|
| `<field>` | `<ctx>` | `<topic.* consumer>` | `<eventually consistent / N s>` |

**The "do-not-write" list:** `<data this service reads but must NEVER mutate, and why>`.

---

## 5. Invariants & Business Rules &nbsp;·&nbsp; CORE

> Rules that must hold for *every* reachable state. State the rule, then the layer that enforces
> it (boundary / domain / store) — an unplaced invariant is an unenforced one.

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | `<rule that must always be true>` | `<domain / gRPC boundary / Lua-atomic in Redis>` | `<error code / Status>` |

<!-- TIER-0/1 aspirational: name the test that proves each invariant, turning this into an
     executable contract. e.g. "I1 — proven by domain::tests::rejects_double_publish". -->

---

## 6. Workflows & Orchestration &nbsp;·&nbsp; DEEP

> How multi-step / cross-context processes run. Distinguish **sync** (in-request, gRPC) from
> **async** (Kafka, eventually consistent). Name the **compensation** for every step that can
> fail after a prior step committed.
> <!-- After C4 is regenerated from docs/domain/, add: "see C4 dynamic view <id>".
>      Until then, describe the flow inline below — do NOT link docs/_legacy/. -->

**`<Workflow name>`** (`<sync \| async saga>`)

1. `<actor/trigger>` → `<step>` (`<consistency: atomic \| best-effort>`)
2. …
- **Compensation:** if step `<n>` fails after step `<n-1>` committed → `<compensating action>`.
- **Idempotency:** `<dedup key / UUIDv5 / claim — must match the README async contract>`.

---

## 7. Context Relationships (Context-Map slice) &nbsp;·&nbsp; DEEP

> This context's edges from `docs/domain/CONTEXT_MAP.md`, stated with the DDD pattern so the
> coupling type is explicit.

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `<ctx>` | upstream | `<Conformist \| ACL \| Customer/Supplier>` | `<topic.* / gRPC>` | `<impact>` |
| `<ctx>` | downstream | `<Published Language \| OHS>` | `<our topic.*/RPC>` | `<their impact>` |

> **Anti-Corruption Layer:** `<the decode/translation module that protects our model from theirs, e.g. infrastructure/decode.rs>`.

---

## 8. Domain Events (semantics, not wire) &nbsp;·&nbsp; DEEP

> The *business meaning* of each event. The wire/proto schema is owned by the README/contract —
> do not duplicate it here. This section answers "what does it MEAN that this happened?".
> Roll-up lives in `docs/domain/EVENT_CATALOG.md`.

| Event | Means (past-tense business fact) | Emitted when (domain trigger) | Who reacts & why |
|---|---|---|---|
| `<topic.thing.happened>` | `<the irreversible business fact it asserts>` | `<the domain transition>` | `<ctx>` — `<reaction>` |

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

> Why the boundary is where it is. Each row links an immutable ADR — never inline the reasoning,
> link it, so the *why* survives refactors.

| Decision | ADR | Status |
|---|---|---|
| `<the locked architectural choice>` | [`ADR-00NN`](../../../../docs/adr/00NN-*.md) | `<Accepted \| Superseded by ADR-00MM>` |

---

## 10. Subdomain Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** `<Core \| Supporting \| Generic>` — `<investment implication>`.
- **Volatility:** `<how often the model is expected to change, and why>`.
- **Known modeling debt:** `<aggregates that are wrong-but-tolerated, with the ADR or ticket>`.
- **Deferred capabilities:** `<modeled-for but not built; the seam left for them>`.
