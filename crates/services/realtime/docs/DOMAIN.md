# `realtime` — Domain & Functional Contract

> **Domain Card**
>
> | | |
> |---|---|
> | **Bounded Context** | Live Delivery — the client-facing System-of-Connection |
> | **Subdomain class** | **Supporting** — it accelerates delivery but owns none of the value; the SoRs (`chat`/`notification`/`counter`) own every byte it forwards. Bespoke (not Generic) because the C10M edge is hand-built |
> | **System of …** | **Connection / Delivery** — explicitly **never** a System of Record |
> | **Aggregate root(s)** | `Connection` (`domain::connection`), with `Session`, `SubscriptionSet`, `SequenceState` |
> | **Tier** | **TIER-1** |
> | **Failure posture** | **Fail-open** — a delivery miss costs latency, never data; clients re-sync from the SoRs on reconnect |
> | **Upstream contexts** | end-user clients over WSS (not internal services); `auth` via `auth-context` (a verify, not a call) |
> | **Downstream contexts** | none of record; delegates offline delivery to `notification` (APNs/FCM) |
> | **Decision log** | [`ADR-0003`](../../../../docs/adr/0003-realtime-is-a-fail-open-system-of-connection.md) |

---

## 1. Business Capability & Non-Goals

**Capability.** `realtime` is the authority for **live client delivery**: it answers
**"which of this user's devices are connected right now, and how do I get this already-durable event
onto the exact device the instant it happens?"**

**The hard problem** is twofold. **Polling's inverted load trap:** at hyperscale, clients polling
the edge couple core QPS to the count of *idle* users — every empty poll pays full TLS + auth +
fan-out to deliver nothing, a self-inflicted DDoS on the mesh. **Connection sprawl:** `chat` and
`notification` each grew their own client streaming, so one device holds multiple sockets with
redundant heartbeats. Realtime inverts the first (work tracks *events*, not eyeballs) and collapses
the second into one multiplexed socket per device.

**Non-goals — what this context deliberately does NOT do:**
- ❌ Store any entity or message → durability lives in `chat`/`notification`/`counter`.
- ❌ Sit in any synchronous write path → it is never in the path of sending a message or persisting a notification.
- ❌ Re-authenticate per frame → the edge token is verified once at the handshake.
- ❌ Author content-level authorization → that happened upstream at emit time; realtime only checks channel-scope ownership.

---

## 2. Ubiquitous Language

| Term | Meaning in this context | Code symbol |
|---|---|---|
| Connection | One live, multiplexed client socket and its lifecycle | `Connection`, `ConnectionState` |
| Session | The authenticated identity bound to a connection at handshake | `Session` |
| Channel | A logical stream multiplexed over the one socket (`dm`, `notif`, `presence`, `counter:<id>`, `feed:<id>`) | `ChannelRef`, `ChannelKey`, `ChannelClass` |
| Subscription set | The channels a connection is subscribed to (capped) | `SubscriptionSet` |
| Stream sequence | The per-stream monotonic token clients dedupe and re-sync from | `StreamSeq`, `SequenceState` |
| Presence | Internal liveness (online/offline) derived from connections — not a product stream | `PresenceState` |
| Targeted vs broadcast | Fan-out that names a *recipient user* vs one that names an *entity* | (dispatcher modes) |
| Node / registry | A gateway instance and the `user → node` routing map | `ConnectionRegistry` port |

---

## 3. Domain Model

| Element | Kind | Invariant boundary it guards |
|---|---|---|
| `Connection` | aggregate root | Lifecycle + the bounded send queue / shed discipline for one socket |
| `Session` | entity | The pinned identity authenticated at handshake |
| `SubscriptionSet` | VO | Channel subscriptions, capped (`RTM-3002`) and scope-checked |
| `SequenceState` / `StreamSeq` | VO | Monotonic per-stream sequencing for client-side dedupe + re-sync |
| `ChannelRef` / `ChannelKey` / `ChannelClass` | VO | A channel's identity and its ownership scope |
| `PresenceState` / `ConnectionState` / `CloseReason` / `DeliveryGuarantee` | enum | The closed lifecycle + delivery vocabulary |

**Connection lifecycle:**

```
handshake (verify token → bind Session) --> Active --(subscribe within scope)--> delivering
     │ heartbeat timeout / shed / drain                                              │
     ▼                                                                               ▼
   Closed  ◄──────────────── reconnect (jittered backoff) ◄──── drain (control frame) ┘
```

> **Legal transitions only.** A connection may subscribe only to channels scoped to its pinned
> identity (Alice → `dm:alice`, never `dm:bob`) — else `RTM-3001`. A slow consumer is **shed**
> (`RTM-5001`), never buffered unboundedly.

---

## 4. Data Ownership & Boundaries

**This context is the source of truth for:**
- Nothing durable. It owns only **ephemeral Redis routing state** — `presence:{user_id}` → node(s) and the node-hop Pub/Sub fabric. All of it is TTL'd and self-healing.

**Everything it forwards is owned elsewhere:**

| Forwarded data | Owned by | Reaches realtime via | Durability |
|---|---|---|---|
| Notifications / badges | `notification` | `notification.v1.events` (targeted) | `notification` persists it |
| Engagement magnitudes | `counter` | `counter.v1.popularity` (broadcast) | `counter` holds it |
| Post lifecycle | `post` | `post.v1.events` (broadcast) | `post` persists it |
| Messages | `chat` | (not consumed — chat runs its own live plane, coexist-first) | `chat` persists it |

**The "do-not-write" list:** realtime writes no SoR; a registry miss (recipient offline) is a
no-op, with the locked-phone path delegated to `notification`.

---

## 5. Invariants & Business Rules

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | The plane stores nothing of record; a registry miss is a no-op | domain + application | (by design) |
| I2 | Authenticate once, at the handshake; never re-verify per frame | infrastructure boundary | `RTM-1001` (handshake) |
| I3 | The only authorization is channel-scope ownership against the pinned identity | domain | `RTM-3001` |
| I4 | A slow consumer is shed, not buffered (bounded per-connection queue) | infrastructure | `RTM-5001` |
| I5 | Fail-open — a lost live event is never a lost message; client re-syncs via `StreamSeq` | application | (recovery, not error) |
| I6 | Subscriptions are capped per connection | domain | `RTM-3002` |

---

## 6. Workflows & Orchestration

> Inline until a corrected C4 is regenerated from `docs/domain/`.

**Connection (handshake → delivery).**
1. Client connects over WSS (`:8443`, behind an L4 LB); the edge token is verified once via `auth-context` → a `Session` is bound.
2. The gateway writes the `user → node` registry entry and subscribes to its node channel.
3. Client subscribes to channels within its identity scope; frames are not re-authenticated thereafter.

**The internal→external bridge (three stages).**
1. **Emit once:** a core service publishes to Kafka — no new synchronous call inward.
2. **Resolve:** `realtime-dispatcher` (under `run_consumer`) resolves the recipient against the registry (targeted) or names a broadcast channel (entity).
3. **Last hop:** publish to the owning node's Redis channel → the gateway hands it to the connection's bounded mailbox → socket write. Core→device = one Kafka hop + one Redis hop + one socket write.
- **Idempotency:** fan-out is naturally idempotent; duplicate live frames are harmless (client dedupes on `StreamSeq`); an unroutable event (`RTM-8002`/`RTM-8003`) folds into `Ok`.

**Two fan-out modes.** *Targeted* (notification/DM/presence) names a recipient → registry-resolve → hop to owning node(s). *Broadcast* (counter/feed) names an entity → publish once to the fleet broadcast channel → every node delivers to its local subscribers.

**Degradation & reaping.** Half-open connections are reaped on heartbeat timeout (`RTM-5002`, frees FD + registry slot, flips presence offline). On node drain, stop-accept → reconnect control frame with jittered backoff (`RTM-5003`) → drain.

---

## 7. Context Relationships (Context-Map slice)

| Neighbour context | Direction | Pattern | Mechanism | What breaks if they change |
|---|---|---|---|---|
| end-user clients | upstream | OHS (Published Language) | the WSS multiplexed envelope `{stream_seq, channel, ack_required, payload}` | an envelope change breaks every client |
| `auth` | dependency | Conformist (verify-only) | `auth-context` ES256 edge-token verification | new handshakes fail if token format changes |
| `notification` | upstream + delegate | ACL | consumes `notification.v1.events`; delegates offline push | decode breaks / offline path lost |
| `counter` | upstream | ACL | consumes `counter.v1.popularity` | broadcast decode breaks |
| `post` | upstream | ACL | consumes `post.v1.events` | broadcast decode breaks |
| `chat` | peer | Separate Ways (coexist) | not consumed — chat owns its own live plane | — (consolidation is a future decision) |

> **Anti-Corruption Layer:** `infrastructure/decode.rs` maps each upstream wire event into the
> internal `DeliverableEvent`; the opaque payload is forwarded, never interpreted.

---

## 8. Domain Events (semantics, not wire)

> Realtime **publishes nothing of record**. Presence, if surfaced, is internal liveness only — not a
> System-of-Record stream.

| Event | Means | Emitted when | Who reacts |
|---|---|---|---|
| — (none of record) | realtime asserts no durable business facts | — | — |

It consumes the facts of `notification`/`counter`/`post`; their meanings are owned by those
contexts' `DOMAIN.md §8` and rolled up in `docs/domain/EVENT_CATALOG.md`.

---

## 9. Decisions & Rationale

| Decision | ADR | Status |
|---|---|---|
| Realtime is a fail-open System-of-Connection, never a record store | [`ADR-0003`](../../../../docs/adr/0003-realtime-is-a-fail-open-system-of-connection.md) | Accepted |
| Coexist-first with `chat`'s live plane (do not consume `chat.message.sent` yet) | _see ADR-0003 consequences_ | Accepted |

---

## 10. Subdomain Classification & Evolution

- **Classification:** Supporting — it makes the product *feel* live but owns no truth; the value lives in the SoRs. Investment goes to the bespoke C10M edge, not to data ownership.
- **Volatility:** low-to-medium — new fan-out sources (channels) are additive; the connection model and envelope are stable.
- **Known modeling debt:** `SIGTERM` → `broadcast_drain` runtime wiring is not yet connected; the internal-gRPC `NodeChannel` backpressure variant is unbuilt (Redis Pub/Sub fabric is the v1).
- **Deferred capabilities:** WebTransport/HTTP-3 transport lane; presence as a product-facing stream; cross-region / multi-PoP routing; the `chat` + `notification` client-streaming consolidation (the seam — reducing them to event producers — is left open).
