# `search` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Search & Discovery — the inverted-index read model |
> | **Subdomain class** | **Supporting** — a derived discovery read-model; owns no source content |
> | **System of …** | **Reference (SoRef)** — eventually-consistent index over profiles/posts/hashtags, rebuildable |
> | **Aggregate root(s)** | `IndexDocument` (`PostDoc` / `ProfileDoc` / `HashtagDoc`) |
> | **Tier** | **TIER-1** |
> | **Failure posture** | **Fail-open** — a degraded index returns fewer/staler hits, never an error |
> | **Upstream contexts** | `post`, `profile`, `moderation` — via **ACL** over Kafka |
> | **Downstream contexts** | clients (search/suggest); publishes none of record |
> | **Decision log** | _none yet — see [`docs/adr/`](../../../../docs/adr/README.md)_ |

---

## 1. Business Capability & Non-Goals

**Capability.** `search` is the authority for **discovery queries**: it answers
**"which profiles/posts/hashtags match this query, ranked, and respecting visibility?"**

**The hard problem.** Keeping an eventually-consistent inverted index correct under out-of-order
events and re-indexing — **OpenSearch as the single canonical store with external versioning** (a
2-version-guard Painless script), a pure-Kafka command side and a stateless read RPC, plus
blue-green reindexing.

**Non-goals — what this context deliberately does NOT do:**
- ❌ Own profiles/posts → it indexes references (SoReference, not SoRecord).
- ❌ Decide visibility → it *honours* the dual visibility authority (owner + moderation).
- ❌ Serve magnitudes → consumes `PopularityScore` from `counter`.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Index document | A searchable doc (post/profile/hashtag) | `IndexDocument`, `PostDoc`, `ProfileDoc`, `HashtagDoc` |
| Doc version | The external version guarding out-of-order updates | `DocVersion` |
| Index mutation | An upsert/delete to apply to the index | `IndexMutation`, `EntityDeletion` |
| Search hit / results | A ranked match and the result set | `SearchHit`, `SearchResults`, `HitDisplay` |
| Suggestion | A typeahead suggestion | `Suggestion`, `Suggestions`, `SuggestQuery` |
| Visibility authority | Who may suppress a doc (owner vs moderation) | `VisibilityAuthority`, `VisibilityChange` |
| Popularity score | The ranking signal from `counter` | `PopularityScore` |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `IndexDocument` | aggregate (per kind) | The searchable projection of a source entity |
| `DocVersion` | VO | External version → out-of-order writes are rejected (2-version guard) |
| `IndexMutation` / `EntityDeletion` | VO | The applied index change |
| `VisibilityAuthority` / `VisibilityChange` | enum/VO | Dual visibility (owner + moderation) honoured in results |
| `SearchQuery` / `SortStrategy` | VO/enum | Query intent + ranking |

> **Invariant.** An update with a lower `DocVersion` than the indexed one is dropped (external
> versioning); a doc is returned only if both visibility authorities allow it.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth (of *reference*) for:**
- The inverted index — **OpenSearch** (single canonical store). Fully rebuildable from upstream via blue-green reindex.

**This context holds derived copies it does NOT own:**

| Copied data | Owned by | Kept fresh via | Staleness tolerance |
|---|---|---|---|
| Post/profile/hashtag content | `post` / `profile` | `post.v1.events` / `profile.v1.events` | eventually consistent |
| Moderation visibility | `moderation` | `moderation.v1.events` | eventually consistent |
| Popularity | `counter` | `counter.v1.popularity` (deferred wiring) | eventually consistent |

**The "do-not-write" list:** search never mutates source entities; it projects them.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Out-of-order updates rejected by external `DocVersion` (2-version guard) | infrastructure (Painless) | `SCH-2xxx` |
| I2 | A hit is returned only if owner AND moderation visibility allow | domain | `SCH-3xxx` |
| I3 | Reads fail open (degrade, never error) | application | `SCH-4xxx` |
| I4 | Reindex is blue-green (no read downtime) | application | `SCH-5xxx` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Index (command side, pure Kafka).** Consume `post.v1.events` (hydrated) + `profile.v1.events` +
`moderation.v1.events` → build `IndexMutation` with an external `DocVersion` → upsert/delete in
OpenSearch under the 2-version Painless guard.

**Query (read side, stateless).** A `SearchQuery` / `SuggestQuery` → ranked OpenSearch query →
filter by dual visibility → return `SearchResults` / `Suggestions`. Fail-open on a degraded cluster.

**Reindex.** Blue-green `Reindexer` rebuilds into a new index and atomically swaps the alias.

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `post` | upstream | ACL | `post.v1.events` (hydrated) | post search goes stale |
| `profile` | upstream | ACL | `profile.v1.events` | profile search (pending rollout) |
| `moderation` | upstream | ACL | `moderation.v1.events` | visibility suppression breaks |
| `counter` | upstream | ACL | `counter.v1.popularity` | popularity ranking (deferred) |
| clients | downstream | OHS | search/suggest RPC | discovery breaks |

> **Anti-Corruption Layer:** the per-source decode (`PostEvent`/`ProfileEvent`/`ModerationEvent`)
> maps foreign wire shapes into `IndexMutation`.

---

## 8. Domain Events (semantics, not wire)

> Publishes **none of record** — it is a read-model. It consumes `post`/`profile`/`moderation`/
> `counter` facts, whose meanings are owned by those contexts.

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| OpenSearch single canonical store with external 2-version guard; pure-Kafka command side + stateless read RPC | [`ADR-0015`](../../../../docs/adr/0015-search-opensearch-single-store-external-versioning.md) | Accepted |
| Dual visibility authority (owner + moderation) honoured at query time | [`ADR-0015`](../../../../docs/adr/0015-search-opensearch-single-store-external-versioning.md) | Accepted |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Supporting — a derived discovery read-model.
- **Volatility:** medium — ranking and doc schema evolve.
- **Known modeling debt:** profile indexing pending the `profile.v1.events` rollout; popularity wiring deferred.
- **Deferred capabilities:** richer ranking; personalized search; cross-entity blended results.
