# ADR-0009: Engagement is Redis-primary with Lua-atomic edges and Kafka write-behind durability

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** engagement; counter (magnitudes); notification, geo-discovery
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Reactions are extremely high-frequency, idempotent toggles (like/unlike). A per-toggle database
round-trip can't keep up, and a non-atomic read-modify-write races under concurrency (double-likes,
lost unlikes). But reactions are still an **edge of record** ("who reacted, how") that must survive
a cache loss.

## Decision

Engagement is **Redis-primary**: each react/unreact is a **Lua-atomic** set/clear of the reaction
edge plus an in-Redis score update (idempotent by construction), with **Kafka write-behind** for
durable recording and downstream propagation (`engagement.reactions`, `engagement.score_updated`).
Engagement owns the **edge**; `counter` owns the derived **magnitudes** (see ADR-0008). The edge is
the truth; the score is derived from `ReactionWeight`s.

## Consequences

- **Positive:** hot-path toggles are atomic and fast with no DB round-trip; durability is preserved
  asynchronously; magnitudes are someone else's job.
- **Negative / accepted trade-off:** a window of write-behind lag where the durable record trails
  Redis; reconciliation/replay depends on the Kafka stream.
- **Closes:** the per-toggle DB bottleneck and reaction race conditions.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Database-primary reactions | Per-toggle round-trip can't sustain reaction volume |
| Non-atomic Redis read-modify-write | Races under concurrency (double/lost reactions) |
| Keep magnitudes here too | Counting belongs in `counter` (ADR-0008) |
