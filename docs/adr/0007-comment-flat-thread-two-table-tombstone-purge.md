# ADR-0007: Comment uses a nil-UUID flat thread, a two-table Scylla layout, and tombstone-vs-purge deletion

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** comment
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Comments are a high-write stream that must be read both by id and in time order, and deletion has
two genuinely different meanings: a user removing their own comment (leave a visible "deleted"
marker) versus a moderation/GDPR hard removal (the row must be gone). One deletion semantic can't
serve both, and a single access pattern can't serve both id-lookup and time-ordered reads cheaply.

## Decision

Comment models a **flat thread using a nil-UUID sentinel** for rootless comments, stores them in a
**two-table ScyllaDB layout** (an LCS table for id lookups + a TWCS table for the time-ordered
stream), and makes deletion an **explicit `DeletionStrategy` — tombstone (visible marker) vs purge
(hard removal)**.

## Consequences

- **Positive:** both read patterns are cheap and compaction-appropriate (LCS vs TWCS); deletion
  semantics match the actual use cases; the nil-UUID sentinel keeps the model flat and simple.
- **Negative / accepted trade-off:** dual-table writes to keep consistent; no nested threading
  (a deliberate simplification).
- **Closes:** the deletion-semantics ambiguity and the read-pattern mismatch.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Single table | Can't serve id-lookup and time-ordered reads with appropriate compaction |
| One "deleted" flag | Conflates user-delete (tombstone) with moderation/GDPR purge |
| Nested thread tree now | Premature complexity for the current product |
