# `comment` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Comments — threaded replies on posts |
> | **Subdomain class** | **Core** — comments are primary UGC engagement |
> | **System of …** | **Record** for comments and their lifecycle |
> | **Aggregate root(s)** | `Comment` (`domain`) |
> | **Tier** | **TIER-1** |
> | **Failure posture** | **Fail-closed on writes** (a posted comment must persist) |
> | **Upstream contexts** | end-user clients; `post` (the commented-on entity) |
> | **Downstream contexts** | `notification`, `engagement`, `counter` — via **Published Language** (`comment.created` / `comment.deleted`) |
> | **Decision log** | _none yet — see [`docs/adr/`](../../../../docs/adr/README.md)_ |

---

## 1. Business Capability & Non-Goals

**Capability.** `comment` is the authority for **comments**: it answers
**"what was commented on which post, by whom, and is it still present or removed?"**

**The hard problem.** Storing a high-write comment stream with a flat-thread model cheaply, and
distinguishing *tombstone* (visible "deleted") from *purge* (hard removal) for moderation/GDPR —
using a **nil-UUID sentinel** for the flat tree and a two-table ScyllaDB layout (LCS for lookups +
TWCS for the time-ordered stream).

**Non-goals — what this context deliberately does NOT do:**
- ❌ Own the post being commented on → `post`.
- ❌ Count comments for display → that's `counter` (magnitudes); comment emits the events.
- ❌ Decide moderation → `moderation` decides; comment applies deletion.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Comment | A reply attached to a post | `Comment`, `CommentId` |
| Comment body | The textual content | `CommentBody` |
| GIF attachment | An attached GIF reference | `GifAttachment` |
| Comment status | Active / tombstoned state | `CommentStatus` |
| Deletion strategy | Tombstone vs purge | `DeletionStrategy` |
| Nil-UUID sentinel | The flat-tree root marker (no parent) | (UUID nil) |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `Comment` | aggregate root | Comment integrity + status transitions |
| `CommentBody` / `GifAttachment` | VO | Content validity at construction |
| `CommentStatus` | enum | Active → tombstoned legality |
| `DeletionStrategy` | enum | Tombstone (visible) vs purge (hard) |
| `PostId` / `ProfileId` | VO | The commented-on entity and author references |

**Lifecycle:**

```
created --(delete: tombstone)--> tombstoned   |   created --(delete: purge)--> purged (hard removed)
```

> **Legal transitions only.** Deletion strategy is explicit; a tombstone preserves the slot, a
> purge removes the row (moderation/GDPR).

---

## 4. Data Ownership & Boundaries

**This context is the source of truth for:**
- Comments — **ScyllaDB** two-table (LCS lookups + TWCS stream). No other service writes them.

**The "do-not-write" list:** comment never writes post state or comment *counts* (those are derived
in `counter` from comment's events).

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | A comment references a valid post + author | domain | `CMT-1xxx` |
| I2 | Tombstone vs purge is an explicit, irreversible choice | domain | `CMT-1xxx` |
| I3 | Flat-tree uses the nil-UUID sentinel for rootless comments | domain | `CMT-1xxx` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Create.** Authorized create → write to both Scylla tables → publish `comment.created` (consumed by
`notification`, `engagement`/`counter` for counts).

**Delete.** Tombstone (visible deleted marker) or purge (hard removal for moderation/GDPR) → publish
`comment.deleted`.

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| `post` | upstream | Customer/Supplier | references `PostId` | orphaned comments if post semantics change |
| `notification` | downstream | Published Language | `comment.created` | reply notifications break |
| `engagement` / `counter` | downstream | Published Language | `comment.created` / `comment.deleted` | comment counts break |

---

## 8. Domain Events (semantics, not wire)

| Event | Means | Emitted when | Who reacts |
|---|---|---|---|
| `comment.created` | a comment was posted on a post | create commits | `notification` (notify author), `counter`/`engagement` (count++) |
| `comment.deleted` | a comment was tombstoned or purged | delete commits | `counter`/`engagement` (count--), feeds |

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| Nil-UUID sentinel flat-tree + two-table (LCS+TWCS) Scylla layout | [`ADR-0007`](../../../../docs/adr/0007-comment-flat-thread-two-table-tombstone-purge.md) | Accepted |
| Tombstone vs purge deletion semantics | [`ADR-0007`](../../../../docs/adr/0007-comment-flat-thread-two-table-tombstone-purge.md) | Accepted |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Core — comments are primary UGC.
- **Volatility:** low-to-medium.
- **Known modeling debt:** flat-thread only (no nested threading).
- **Deferred capabilities:** nested threads; richer attachments.
