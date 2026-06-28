# ADR-0002: Moderation is the decision/enforcement SoR with a narrow fail-closed Screen gate

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** moderation (new TIER-0 context); content producers; audit (consumer)
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Trust & safety logic today would scatter across every content service — each re-implementing "is
this actioned?" checks with no authoritative answer. We need a single **brain and system of record
for integrity decisions and enforcement**. The tension: such a service must be able to *block*
harmful content authoritatively, yet most of its work (ingesting signals, recording decisions)
must not sit on the synchronous write path as a latency or availability liability. A service that
is fail-open everywhere lets harmful content through during an outage; one that is fail-closed
everywhere makes all publishing hostage to its uptime.

Moderation must also be carefully scoped: it is **not** a classifier (that couples policy to the ML
lifecycle), **not** a content store, and **not** a review UI.

## Decision

We build **`moderation` as the TIER-0 decision/enforcement SoR** exposing a **three-plane
interface**, each plane with the failure posture its semantics demand:

1. **Async ingestion** — signals/reports flow in over Kafka (`run_consumer`, fail-open). This is
   the bulk path and never blocks producers.
2. **Hot-read enforcement** — "is this entity actioned?" served from a fast read path, fail-open
   (absence of a record ≠ blocked).
3. **Narrow fail-closed `Screen` gate** — a single synchronous pre-publish check for screened
   categories. If moderation is unavailable, `Screen` **denies** — content is not published.

Moderation records each decision once and emits a dedicated `DecisionRecorded` evidence event so
the [audit plane](./0001-audit-is-a-separate-evidence-plane.md) can seal the DSA rationale;
existing offender-centric enforcement events are left untouched.

## Consequences

- **Positive:** one authoritative answer to "is this actioned?"; the expensive paths stay async
  and fail-open, so moderation load never amplifies onto producers; only the deliberately narrow
  `Screen` gate is fail-closed; clean separation from classifier / store / UI keeps policy
  decoupled from ML.
- **Negative / accepted trade-off:** `Screen` is a synchronous dependency on the publish path — it
  must be fast and carry a tight timeout, and a moderation outage blocks *new* content in screened
  categories (an accepted safety-over-availability trade for that narrow surface).
- **Closes:** scattered, non-authoritative moderation logic across content services.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Make `Screen` fail-open | Lets harmful content through precisely during an outage — unacceptable for T&S |
| Moderation as a classifier | Couples integrity policy to the ML model lifecycle; conflates "decide" with "detect" |
| Embed moderation state in each content service | No authoritative SoR, no consistent enforcement, no single evidence trail |
