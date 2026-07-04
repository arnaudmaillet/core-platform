# ADR-0016: Social-graph uses a 4-table Scylla schema with logged-batch atomic dual-writes

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** social-graph; downstream timeline, counter; profile (tier)
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

A relation is inherently bidirectional to query — "who do I follow" and "who follows me" — so each
follow/block needs a forward and a reverse index. If those two writes can diverge, the graph
corrupts (a follow visible one way but not the other). Hot reads (timeline fan-out) need the
follower set fast, and a block must atomically sever existing follows both ways. Author tier derives
from follower count but is *presented* on the profile.

## Decision

`social-graph` is the **relations System of Record** on a **4-table ScyllaDB schema** (forward +
reverse indexes for follows and blocks) with **Redis hot Sets** for follower reads, and writes the
forward + reverse rows in a **logged batch** so the dual-write is atomic. A block severs existing
follows (`SeveredFollows`). **Author tier is computed here** from follower count crossing
`TierThresholds`, but **owned and emitted by `profile`** (ADR-0014). `timeline`/`counter` read the
graph over gRPC (the `social-graph.follows` Kafka producer is deferred).

## Consequences

- **Positive:** forward/reverse indexes can't diverge; follower reads are hot; blocks are consistent;
  tier computation lives where the follower data is.
- **Negative / accepted trade-off:** logged batches cost more than independent writes; orphan
  reconciliation is still a tracked debt; consumers read via gRPC until the follows stream lands.
- **Closes:** the divergent-index corruption risk and slow follower reads.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Single forward-only table | Reverse queries ("who follows me") become full scans |
| Non-atomic forward/reverse writes | Indexes diverge → graph corruption |
| Emit tier from social-graph | Tier is *presented* on the profile (ADR-0014) |
