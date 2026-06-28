# Context Map

> 🟡 **Scaffold.** This file is the cross-context relationship map for the platform. It is
> **multi-owner** — no single crate can keep it true — so it lives here, owned by the
> architecture guild. Fill it from ground truth (the crates, their `*.v1` protos, and the
> event-topology registry guard), **never** from the quarantined legacy C4.

## What this document is

A [DDD **context map**](https://martinfowler.com/bliki/BoundedContext.html): the bounded
contexts and the *type of coupling* on every edge between them. The coupling type — not just
"A talks to B" — is the point, because it tells a reviewer what breaks when a neighbour changes.

## Relationship patterns (the vocabulary)

| Pattern | Meaning in this codebase |
|---|---|
| **Open-Host Service / Published Language (OHS/PL)** | A context publishes a stable contract for all comers — our `*.v1` protos and Kafka topics |
| **Anti-Corruption Layer (ACL)** | A consumer translates a foreign schema into its own model at the edge — e.g. `infrastructure/decode.rs` wire→domain mappers |
| **Conformist** | A downstream accepts an upstream's model as-is, no translation |
| **Customer / Supplier** | Downstream's needs are negotiated into the upstream's roadmap |
| **Shared Kernel** | A shared model both contexts depend on (use sparingly — e.g. shared foundation crates) |
| **Partnership** | Two contexts succeed or fail together and coordinate releases |

## Subdomain classification

Each context is **Core**, **Supporting**, or **Generic** — this drives investment. Record it in
each service's Domain Card; this table is the roll-up.

| Context | Subdomain class | Notes |
|---|---|---|
| `<ctx>` | `<Core \| Supporting \| Generic>` | `<one line>` |

## The map

> Fill one row per directed edge. Direction is from the perspective of the **owning** context.

| Upstream context | Downstream context | Pattern | Mechanism (topic / RPC) | What breaks downstream if upstream changes |
|---|---|---|---|---|
| `<ctx>` | `<ctx>` | `<OHS/PL \| ACL \| Conformist \| …>` | `<topic.* \| pkg.v1.Rpc>` | `<impact>` |

## Diagram

> A C4 Container/Component view will be **regenerated from this map** (and the per-service Domain
> Cards) once the scaffold is filled. Until then, this table is authoritative. Do **not** link
> the legacy C4 in `docs/_legacy/`.
