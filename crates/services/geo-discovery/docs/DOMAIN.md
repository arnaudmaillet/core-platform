# `geo-discovery` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Geo Discovery — spatial discovery of posts on a map |
> | **Subdomain class** | **Supporting** — a derived spatial read-model; product-distinctive surface but owns no truth |
> | **System of …** | **Reference (SoRef)** — a queryable spatial index over posts, rebuildable from upstream |
> | **Aggregate root(s)** | `MapPostCard` (projection) keyed by `H3Index` |
> | **Tier** | **TIER-1** |
> | **Failure posture** | **Fail-open** — a degraded index returns fewer/staler cards, never an error |
> | **Upstream contexts** | `post` (published posts w/ location), `engagement` (virality), `profile`/`social-graph` (author tier) — via **ACL** |
> | **Downstream contexts** | clients (map viewport queries); publishes none of record |
> | **Decision log** | _none yet — see [`docs/adr/`](../../../../docs/adr/README.md)_ |

---

## 1. Business Capability & Non-Goals

**Capability.** `geo-discovery` is the authority for **spatial discovery**: it answers
**"what are the most relevant posts visible in this map viewport right now?"**

**The hard problem.** Serving viewport queries at interactive latency over a constantly-changing
post population — an **H3 `grid_disk` viewport** mapped to a dual-layer Redis structure (ZSET +
cardinality), with Lua Top-K / XX / prune scripts and TTL'd retention so the index self-trims.

**Non-goals — what this context deliberately does NOT do:**
- ❌ Own posts → `post` is the SoR; geo holds a spatial projection.
- ❌ Compute engagement scores → consumes them from `engagement`.
- ❌ Own author tier → consumes `profile.tier_changed`.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Map post card | The projected, map-renderable summary of a post | `MapPostCard` |
| H3 index / resolution | The hexagonal spatial cell id and its resolution | `H3Index`, `H3Resolution` |
| Geo coordinate | A lat/lng point | `GeoCoordinate` |
| Virality score | The engagement-derived ranking weight | `ViralityScore` |
| Author tier | The author's tier (affects ranking/visibility) | `AuthorTier` |
| Retention TTL | How long a card stays in the spatial index | `RetentionTtl` |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `MapPostCard` | projection (aggregate) | The map-renderable post summary in a cell |
| `H3Index` / `H3Resolution` | VO | Spatial cell identity + zoom granularity |
| `GeoCoordinate` | VO | Valid lat/lng at construction |
| `ViralityScore` / `AuthorTier` | VO/enum | Ranking inputs |
| `RetentionTtl` | VO | Self-trimming lifetime |

> **Invariant.** A card lives in exactly the H3 cell(s) for its coordinate; ranking within a cell is
> Top-K by virality, pruned and TTL'd.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth (of *reference*) for:**
- The spatial index — **Redis** (ZSET + cardinality dual-layer) + **ScyllaDB** (`map_post_cards`). Rebuildable from upstream events.

**This context holds derived copies it does NOT own:**

| Copied data | Owned by | Kept fresh via | Staleness tolerance |
|---|---|---|---|
| Post content/location | `post` | `post.published` / `post.deleted` | eventually consistent |
| Virality | `engagement` | `engagement.score_updated` | eventually consistent |
| Author tier | `profile` | `profile.tier_changed` | eventually consistent |

**The "do-not-write" list:** geo never mutates posts, scores, or tiers — it indexes them.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | A card is indexed in the correct H3 cell for its coordinate | domain | `GEO-1xxx` |
| I2 | Within a cell, results are Top-K by virality, pruned + TTL'd | domain (Lua) | `GEO-2xxx` |
| I3 | Viewport queries fail open (degrade, never error) | application | `GEO-1xxx` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Index maintenance.** Consume `post.published` (add card), `post.deleted` (remove),
`engagement.score_updated` (re-rank), `profile.tier_changed` (re-weight) → update the dual-layer
Redis ZSET via Lua Top-K/XX/prune; TTL handles retention.

**Viewport query.** A map viewport → `H3 grid_disk` of covering cells → merge Top-K per cell →
return `MapPostCard`s. A degraded index returns fewer/staler cards (fail-open).

> **Known payload gap:** `post` currently emits no lat/lng/caption on `post.published`, so the geo
> projection depends on a product decision to enrich the post event (recorded in the pre-infra audit).

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `post` | upstream | ACL | `post.published` / `post.deleted` | cards stop appearing/clearing |
| `engagement` | upstream | ACL | `engagement.score_updated` | ranking goes stale |
| `profile` | upstream | ACL | `profile.tier_changed` | tier weighting breaks |
| clients | downstream | OHS | viewport gRPC query | map discovery breaks |

> **Anti-Corruption Layer:** the consumers translate each upstream event into `MapPostCard` updates.

---

## 8. Domain Events (semantics, not wire)

> Publishes **none of record** — it is a read-model. It consumes the facts of `post` / `engagement`
> / `profile`; their meanings are owned by those contexts.

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| H3 grid_disk viewport + dual-layer Redis (ZSET+cardinality) Top-K spatial index | _candidate — not yet recorded_ | — |
| Post→geo payload enrichment (lat/lng/caption) needs a product decision | _open — see pre-infra audit_ | Open |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Supporting — a distinctive but derived spatial projection.
- **Volatility:** medium — ranking inputs evolve.
- **Known modeling debt:** the post→geo payload gap (no lat/lng/caption emitted upstream).
- **Deferred capabilities:** richer spatial queries; clustering; heatmaps.
