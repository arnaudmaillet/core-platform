# ADR-0015: Search is an OpenSearch read-model with external versioning and a fail-open read path

- **Status:** Accepted
- **Date:** 2026-06-26
- **Context(s) affected:** search; upstream post, profile, moderation, counter
- **Deciders:** arnaudmaillet (architecture)

## Context and problem

Discovery needs an inverted index over profiles/posts/hashtags, fed by an at-least-once,
**out-of-order** event stream. A late or duplicated event must not clobber a newer document, the
index must respect both owner and moderation visibility, reindexing must not cause read downtime,
and a degraded search cluster must not take down the calling surfaces.

## Decision

`search` is an eventually-consistent **read-model (SoReference)** on **OpenSearch as the single
canonical store**, with a **pure-Kafka command side** and a **stateless read RPC**. Out-of-order
writes are guarded by **external `DocVersion`** (a 2-version Painless script rejects stale updates).
Results honour **dual visibility authority** (owner + moderation). Reindexing is **blue-green** (build
new index, atomically swap the alias — no read downtime). The read path **fails open** (degrade to
fewer/staler hits, never error).

## Consequences

- **Positive:** out-of-order events can't regress a document; no read downtime on reindex;
  visibility is enforced at query time; a degraded cluster degrades search, not the product.
- **Negative / accepted trade-off:** search is eventually consistent; OpenSearch is a single store
  (no second source of truth — it's rebuildable from upstream instead).
- **Closes:** stale-overwrite races, reindex downtime, and search-outage blast radius.

## Alternatives rejected

| Option | Why rejected |
|---|---|
| Trust event order | At-least-once + reordering corrupts the index |
| In-place reindex | Read downtime during rebuild |
| Fail-closed reads | A search outage would break the calling surfaces |
