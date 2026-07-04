# ADR-0008: Counter is a reconcilable System-of-Reference for magnitudes, distinct from edge state

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** counter; supersedes engagement's raw counts; producer for search, realtime
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

"How many views/likes/followers?" (a **magnitude**) and "who liked/followed whom?" (**edge state**)
look related but have opposite shapes: magnitudes are a firehose-scale aggregate that tolerates
approximation and reconciliation; edge state is exact relational truth. Storing magnitudes inside
the edge-state services (engagement, social-graph) couples a high-write counting workload to the
relational write path and scatters the same count across services.

## Decision

`counter` is a dedicated **System of Reference for magnitudes** — exact-but-reconcilable, never edge
state. It ingests a pure-Kafka firehose with windowed N→1 pre-aggregation, stores across a 3-tier
hot/warm/cold layout (Redis / Postgres SoRef+reconciliation / Scylla TWCS), uses HLL for unique
counts and CMS for trending, and gates all-tier side effects behind a **`WindowId`-keyed
idempotency ledger** so replays can't double-count. It **supersedes engagement's raw view/share
counts** and **reconciles** authoritative totals against the owning SoRs (drift → `CTR-5002`).
Reads **fail open** (hard timeout → stale/approximate).

## Consequences

- **Positive:** counting scales independently of the edge SoRs; one home per magnitude; replay-safe;
  hot reads never block the product.
- **Negative / accepted trade-off:** magnitudes are eventually consistent and reconciled, not
  transactionally exact at the instant of read; reconciliation sources (e.g. follower counts) must
  be exposed by the edge SoRs.
- **Closes:** scattered counters and the high-write-counting-on-the-relational-path coupling.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Keep counts in engagement/social-graph | Couples firehose counting to the relational write path; duplicates counts |
| Exact transactional counters | Doesn't scale to firehose volume; melts the hot path |
| Approximate-only (no reconciliation) | Drifts unboundedly from the authoritative edge truth |
