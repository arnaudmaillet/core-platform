# ADR-0010: Geo-discovery is an H3 grid_disk + dual-layer Redis Top-K spatial read-model

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** geo-discovery; upstream post, engagement, profile
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Map viewport queries must return the most relevant posts in a moving rectangle at interactive
latency, over a post population that constantly changes and must self-trim (stale posts shouldn't
linger). Arbitrary lat/lng range queries don't index well, and an unbounded per-cell list both
costs memory and returns junk.

## Decision

Geo-discovery is a **fail-open spatial read-model (SoReference)** built on **H3**: a viewport maps
to an `H3 grid_disk` of covering cells; each cell is a **dual-layer Redis structure (ZSET +
cardinality)** maintained by Lua **Top-K / XX / prune** scripts with **TTL'd retention** so the
index self-trims. Cards are projected from upstream events (`post.published`/`post.deleted`,
`engagement.score_updated`, `profile.tier_changed`); geo owns no source truth and a degraded index
returns fewer/staler cards rather than erroring.

## Consequences

- **Positive:** viewport queries become a bounded set of cell lookups; per-cell results are ranked
  and capped; retention is automatic via TTL; the index is fully rebuildable from upstream.
- **Negative / accepted trade-off:** results are eventually consistent and approximate (Top-K);
  depends on upstream emitting the needed payload (see the open post→geo enrichment gap).
- **Closes:** the spatial-indexing and unbounded-cell-list problems.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| lat/lng range queries on a generic store | Poor spatial locality; slow at viewport scale |
| Unbounded per-cell lists | Memory blowup; returns low-relevance noise |
| Make geo a SoR | It's a projection; durability lives in `post` |
