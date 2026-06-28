# `engagement` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Engagement — reactions and their edge state / scoring |
> | **Subdomain class** | **Core** — direct user interaction with content; the reaction edge is product fabric |
> | **System of …** | **Record** for reaction edge state ("who reacted, how"); magnitudes are superseded by `counter` |
> | **Aggregate root(s)** | `Reaction` (`domain`) |
> | **Tier** | **TIER-1** |
> | **Failure posture** | **Fail-open-ish** — Redis-primary with Lua atomicity, Kafka write-behind |
> | **Upstream contexts** | end-user clients; `comment` (counts) |
> | **Downstream contexts** | `counter` (magnitudes), `notification`, `geo-discovery` (score) — via **Published Language** |
> | **Decision log** | _none yet — see [`docs/adr/`](../../../../docs/adr/README.md)_ |

---

## 1. Business Capability & Non-Goals

**Capability.** `engagement` is the authority for **reactions**: it answers
**"who reacted to this content, with what reaction, and what is the weighted engagement score?"**

**The hard problem.** Applying high-frequency, idempotent reaction toggles atomically at the hot
path — **Redis-primary with Lua atomicity** — while durably recording the edge via **Kafka
write-behind**, without a per-toggle database round-trip.

**Non-goals — what this context deliberately does NOT do:**
- ❌ Serve display *counts* → `counter` owns magnitudes; engagement emits the edge events.
- ❌ Own the content reacted to → `post` / `comment`.
- ❌ Own raw view/share counters → superseded by `counter`.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Reaction | A user's reaction edge on a content item | `Reaction`, `ReactionKind` |
| Reaction weight | The score weight of a reaction kind | `ReactionWeight` |
| Upsert / remove | Idempotent set/clear of a reaction | `ReactionUpsertedEvent`, `ReactionRemovedEvent` |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `Reaction` | aggregate root | One reaction edge per (user, content); idempotent toggle |
| `ReactionKind` | enum | Closed reaction vocabulary |
| `ReactionWeight` | VO | Scoring contribution per kind |
| `PostId` / `ProfileId` | VO | The reacted-on content + reactor |

**Lifecycle:**

```
(none) --(react)--> upserted --(react again, same kind)--> idempotent --(unreact)--> removed
```

> **Legal transitions only.** Re-applying the same reaction is idempotent (Lua-atomic in Redis); the
> edge is the truth, the score is derived.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth for:**
- Reaction edge state — **Redis** (primary, Lua-atomic) with **ScyllaDB** durable write-behind.

**The "do-not-write" list:** engagement does not write display counts (emits events; `counter`
aggregates), and does not own the content.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | One reaction edge per (user, content); toggles are idempotent | domain + Lua-atomic in Redis | `ENG-2xxx` |
| I2 | The edge is authoritative; the score is derived from weights | domain | `ENG-3xxx` |
| I3 | Durable record via Kafka write-behind (no per-toggle DB round-trip) | application | `ENG-5xxx` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**React / unreact.** A Lua script atomically sets/clears the reaction edge and updates the
in-Redis score; a `ReactionUpsertedEvent` / `ReactionRemovedEvent` is emitted (Kafka write-behind)
for durable recording and downstream consumption.

**Score propagation.** `engagement.score_updated` carries the weighted score to consumers
(`geo-discovery` virality, `counter`).

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `comment` | upstream | ACL | `comment.created` / `comment.deleted` | comment-driven counts break |
| `counter` | downstream | Published Language | reaction events | like/reaction magnitudes break |
| `notification` | downstream | Published Language | `engagement.reactions` | reaction notifications break |
| `geo-discovery` | downstream | Published Language | `engagement.score_updated` | virality scoring breaks |

---

## 8. Domain Events (semantics, not wire)

| Event | Means | Emitted when | Who reacts |
|---|---|---|---|
| `engagement.reactions` (`ReactionUpserted`/`Removed`) | a reaction edge was set/cleared | react/unreact commits | `notification`, `counter` |
| `engagement.score_updated` | the weighted engagement score changed | score recompute | `geo-discovery`, `counter` |
| `engagement.post_reactions` / `post_interaction_counters` | per-post reaction/interaction rollups | aggregation | downstream consumers |

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| Redis-primary Lua-atomic reactions + Kafka write-behind durability | _candidate — not yet recorded_ | — |
| Engagement keeps the reaction *edge*; `counter` supersedes raw magnitudes | _see counter §4_ | Accepted |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Core — direct content interaction.
- **Volatility:** low-to-medium — new reaction kinds are additive.
- **Known modeling debt:** a reaction-count RPC for `counter` reconciliation is not yet exposed.
- **Deferred capabilities:** richer reaction analytics; per-kind scoring tuning.
