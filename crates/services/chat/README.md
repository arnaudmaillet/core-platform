# `chat` — Hyperscale conversations where a private group can go viral without melting

## 🎯 Overview & Service Role

`chat` is the unified **Conversation** microservice for the platform: it powers both **Group chats** (symmetric `N↔N` mesh, bounded, with presence/typing/read‑receipts) and **Channels** (asymmetric `1→N` broadcast, passive reading, unbounded audience) — behind a single domain model and a single gRPC surface.

The hard problem it solves is the **hybrid I/O profile**: an admin can flip a private Group to **Public**, after which millions of passive guests may read history and subscribe to new messages *while the core members keep interacting in real time*. Done naively, those guests cause **write amplification** and **ScyllaDB hot‑partitioning** on the exact partition the members are actively writing.

The service resolves this with the **Shadowing Pattern**: one logical conversation projects onto two physically isolated runtime planes.

| | **Member Plane** | **Audience Plane** |
|---|---|---|
| Cardinality | bounded (≤ 500) | unbounded (→ millions) |
| Direction | full‑duplex `N↔N` | read‑only `1→N` |
| Carries | messages **+** presence + typing + receipts | message **shadow** only |
| Per‑recipient writes | receipts only, `O(members)` | **zero** |

**Core objectives:** members never feel the audience; guests never touch the member write/presence loops; one durable message write fans out to *pods* (hundreds), never *subscribers* (millions).

---

## 📐 Architecture & Concepts

Hexagonal / DDD layout (`domain` → `application` → `infrastructure`), CQRS command/query buses, ScyllaDB for the durable log, Redis Cluster for cache + real‑time routing, Kafka for events.

```
                       ┌─────────────────────── gRPC (tonic) ───────────────────────┐
   member client ─────▶│ StreamConversation (full)        SendMessage / GetHistory… │◀──── guest client
   guest  client ─────▶│ StreamPublic (shadow)            ToggleVisibility / Sub…    │
                       └───────────────┬───────────────────────────┬────────────────┘
                                       │ CQRS                       │ real-time fork
                            ┌──────────▼──────────┐      ┌──────────▼───────────────┐
                            │ Command / Query bus │      │ MessageFanout            │
                            └──────────┬──────────┘      │  ├─ hot-tail cache push  │
                durable write          │                 │  ├─ member channel (all) │
            ┌──────────────────────────▼───┐             │  └─ audience shards(msg) │
            │ ScyllaDB                      │             └──────────┬───────────────┘
            │  messages_by_conversation     │                        │ SPUBLISH
            │   PK (conversation_id, bucket)│             ┌──────────▼───────────────┐
            │  members / subscriptions      │             │ Redis Cluster (sharded   │
            └───────────────────────────────┘            │ pub/sub + cache)         │
                                                          │  {conv:<id>} member slot │
   Kafka ◀── KafkaEventPublisher (chat.*)                │  {aud:<id>:<k>} spread   │
   Kafka ──▶ VisibilityWorker (unpublish → close guests) └──────────┬───────────────┘
                                                                     │ SSUBSCRIBE (refcounted)
                                                          ┌──────────▼───────────────┐
                                                          │ PlaneSubscriber (per pod) │
                                                          │  → ConversationRegistry   │
                                                          │     (member | audience)   │
                                                          │  → local broadcast to     │
                                                          │     gRPC streams          │
                                                          └───────────────────────────┘
```

**Hot‑partition isolation.** `messages_by_conversation` uses a **composite `(conversation_id, bucket)` partition key** (`bucket = floor(created_at_ms / CHAT_MESSAGE_BUCKET_HOURS)`). Members write the *current* bucket (live tail); guests scroll *older* buckets (cold partitions, often different replicas) — the write hotspot and the bulk read load are physically segregated by bucket age. New‑message reads for guests never hit Scylla at all; they arrive over the broadcast plane.

### Resilience Guarantees & High‑Load Behavior (Critical for Hyperscale)

- **Backpressure (streams):** each plane has its own per‑conversation `tokio::sync::broadcast` channel (`CHAT_MEMBER_STREAM_BUFFER_SIZE` / `CHAT_AUDIENCE_STREAM_BUFFER_SIZE`). A slow consumer that lags is dropped with `Status::data_loss`; the client reconnects and re‑polls `GetHistory`. **Member and audience backpressure are fully independent** — a slow guest can never stall a member.
- **Fan‑out cost = pods, not subscribers.** Redis `SPUBLISH` delivers each event once per *shard*; each pod re‑fans in‑process to its many local streams. Pod Redis subscriptions are **reference‑counted** (subscribe‑on‑first / unsubscribe‑on‑last). Guest‑only pods never subscribe to a member channel, so presence/typing/receipt churn never crosses to audience nodes.
- **Read offload / buffer limits:** the per‑conversation **hot‑tail cache** (capped Redis ZSET, `CHAT_HOT_TAIL_CACHE_SIZE`) serves "last screen" + short scroll, keeping passive readers off the live write partition. A cold/empty cache is always safe — Scylla is the durable source of truth.
- **Dependency failures:** real‑time fan‑out is **best‑effort** — `SendMessage` returns success once the message is durably written; a Redis/broadcast failure only logs. Kafka consumers follow the mandatory runner contract: manual commit after a terminal outcome, **bounded retry with backoff + jitter**, **dead‑letter** on exhaustion/poison, and rebuild‑from‑last‑committed‑offset on broker error.
- **Memory management:** per‑plane registries run a periodic **reaper** that drops zero‑receiver senders; stream **drop guards** release the Redis subscription, presence, and shard activation the instant a client disconnects. Presence/typing/shard‑liveness are expiring sorted sets, so a crashed pod's state ages out without explicit cleanup.
- **Timeouts / consistency tiering:** member writes use the Scylla **Strict** profile (LocalQuorum); guest history reads use **Fast** (LocalOne + speculative execution) to spread load and mask a stalled replica; admin/analytics scans use **Analytical** (Quorum).
- **Slot safety:** all per‑conversation Member‑Plane Redis keys share the `{conv:<id>}` hash tag (single slot, single round‑trip, no `CROSSSLOT`); audience channels use spreading `{aud:<id>:<k>}` tags so a viral conversation is not pinned to one node.

---

## 🔌 Public Interfaces & API Contract

### gRPC — `chat.v1.ChatService`

```protobuf
service ChatService {
  // Lifecycle / membership
  rpc CreateConversation (CreateConversationRequest) returns (CreateConversationResponse);
  rpc ToggleVisibility   (ToggleVisibilityRequest)   returns (CommandResponse);
  rpc JoinAsMember       (JoinAsMemberRequest)       returns (CommandResponse);
  rpc Subscribe          (SubscribeRequest)          returns (CommandResponse);
  rpc Unsubscribe        (UnsubscribeRequest)        returns (CommandResponse);
  // Messaging
  rpc SendMessage (SendMessageRequest) returns (SendMessageResponse);
  rpc MarkRead    (MarkReadRequest)    returns (CommandResponse);
  // Member-Plane signals
  rpc SendTyping (SendTypingRequest) returns (CommandResponse);
  rpc Heartbeat  (HeartbeatRequest)  returns (CommandResponse);
  // Queries
  rpc GetHistory        (GetHistoryRequest)        returns (GetHistoryResponse);
  rpc ListMembers       (ListMembersRequest)       returns (ListMembersResponse);
  rpc ListSubscriptions (ListSubscriptionsRequest) returns (ListSubscriptionsResponse);
  // Real-time streams
  rpc StreamConversation (StreamConversationRequest) returns (stream StreamConversationResponse); // members
  rpc StreamPublic       (StreamPublicRequest)       returns (stream StreamPublicResponse);       // audience
}
```

> **Enum contract:** proto enum values are **0‑based and equal the domain `tinyint`** (`CONVERSATION_KIND_GROUP=0`, `…CHANNEL=1`; `VISIBILITY_PRIVATE=0`, `…PUBLIC=1`; `ROLE_OWNER=0…GUEST=4`; `CONTENT_TYPE_TEXT=0…SYSTEM=2`). No `UNSPECIFIED` sentinel — the gRPC layer casts directly with no off‑by‑one mapping.

**Invariants enforced at the boundary:** `StreamConversation` requires roster membership (`PERMISSION_DENIED` otherwise); `StreamPublic` requires `visibility == Public` (`FAILED_PRECONDITION` otherwise); the audience stream is structurally incapable of carrying presence/typing/receipts.

### Rust ports (hexagonal contract)

```rust
#[async_trait] pub trait MessageRepository {
    async fn insert(&self, message: &Message) -> Result<(), ChatError>;
    async fn list_history(
        &self, conversation_id: &ConversationId, limit: i32,
        cursor: Option<(i64, Uuid)>,          // (created_at_ms, message_id)
        floor_created_at_ms: Option<i64>,     // audience watermark — pushed into Scylla as `created_at >= ?`
    ) -> Result<(Vec<MessageSummary>, Option<(i64, Uuid)>), ChatError>;
}

#[async_trait] pub trait EventPublisher {            // KafkaEventPublisher in prod
    async fn publish_conversation(&self, event: &DomainEvent) -> Result<(), ChatError>;
    async fn publish_message(&self, event: &MessageEvent) -> Result<(), ChatError>;
}
```

**Error contract:** every fault implements `error::AppError` with a stable code — `CHT-1xxx` lifecycle, `CHT-2xxx` validation, `CHT-3xxx` events, `CHT-4xxx` streaming, `CHT-9xxx` identifiers — mapped to gRPC `Status` and HTTP status by the shared `error` crate.

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# Cargo.toml (workspace member)
[dependencies]
chat = { path = "crates/services/chat" }
```

The crate is **library‑only**. It plugs into the shared fleet runtime by
implementing [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `chat::service::ChatService` — `build` wires every adapter (and spawns the
per-pod plane subscriber, registry reapers, and VisibilityWorker), `register`
adds the gRPC services, and `health_probes` exposes the Scylla/Redis liveness
checks. Telemetry, config + hot-reload, ingress rate-limiting, health, and
graceful shutdown are all owned by the runtime.

### Standard Bootstrap Pattern

The deployable binary is `crates/apps/chat-server`, and it is the entire
entrypoint:

```rust
use std::net::SocketAddr;

use chat::service::ChatService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("CHAT_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50051".to_owned())
        .parse()?;

    // Owns telemetry, infrastructure.toml load + hot-reload, the inbound-trace
    // and traffic layers, dynamic gRPC health, and SIGINT-drained shutdown — then
    // builds `ChatService` and serves until shutdown.
    service_runtime::serve::<ChatService>(addr).await
}
```

Apply the ScyllaDB migrations (`crates/services/chat/migrations/0001…0006.cql`) against the `chat` keyspace before first start.

---

## ⚙️ Configuration & Runtime Environment

### Chat‑specific variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `CHAT_MAX_PAGE_SIZE` | No | `50` | Server‑enforced cap on `GetHistory`/`ListSubscriptions` page size (prevents full‑partition scans). |
| `CHAT_HOT_TAIL_CACHE_SIZE` | No | `200` | Messages kept in the per‑conversation Redis hot‑tail cache (read offload). |
| `CHAT_MESSAGE_BUCKET_HOURS` | No | `24` | Time‑bucket width for the Scylla message partition key. **Must be identical cluster‑wide** (writer and reader derive the bucket from it). |
| `CHAT_MEMBER_STREAM_BUFFER_SIZE` | No | `256` | `broadcast` capacity per active Member‑Plane stream; overflow ⇒ `Lagged`. |
| `CHAT_AUDIENCE_STREAM_BUFFER_SIZE` | No | `1024` | `broadcast` capacity per active Audience‑Plane stream (sized larger for fan‑out bursts). |
| `CHAT_AUDIENCE_SHARD_COUNT` | No | `16` | Number of Audience‑Plane sharded channels a public conversation spreads across. |
| `CHAT_PRESENCE_TTL_SECS` | No | `30` | Presence liveness window (also reused as the audience‑shard heartbeat TTL). |
| `CHAT_TYPING_TTL_SECS` | No | `6` | Typing‑indicator expiry (short by design). |

### Inherited infrastructure variables (consumed by shared clients)

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_CONTACT_POINTS` | **Yes** | — | ScyllaDB seed nodes (host:port, comma‑separated). |
| `SCYLLA_LOCAL_DC` | **Yes** | — | Local datacenter for token/DC‑aware routing. |
| `SCYLLA_KEYSPACE` | No | `chat` | Keyspace (see migrations). |
| `REDIS_HOSTS` | **Yes** | — | Redis Cluster nodes (host:port, comma‑separated). |
| `KAFKA_BROKERS` | **Yes** | — | Kafka bootstrap brokers. |
| `KAFKA_SECURITY_PROTOCOL` / `KAFKA_SASL_*` | No | plaintext | Auth for managed Kafka. |

> Full connection/timeout/reconnect tuning (`SCYLLA_*`, `REDIS_*`, `KAFKA_*`) is documented in the `scylla-storage`, `redis-storage`, and `transport` shared crates.

### Compile‑time features

- `fred` is built with `["partial-tracing", "i-scripts"]`; the shared `redis-storage` transitively enables fred's **`subscriber-client`** (required for `SSUBSCRIBE`/`SPUBLISH`).
- `build.rs` compiles the protobuf contract (`proto/chat/v1/*.proto`) and emits a reflection descriptor set.

---

## 📈 Telemetry, Performance & Metrics

- **Runtime prerequisites:** a multi‑threaded **Tokio** runtime (long‑lived streaming tasks, background reapers, Kafka consumers, per‑stream heartbeat tasks). The global tracing/OTel subscriber must be installed before `serve` (clients attach lifecycle + W3C trace‑context propagation across the Kafka boundary).
- **Distributed tracing:** the Kafka producer injects `traceparent`/`tracestate`; consumers re‑establish the parent span, giving end‑to‑end traces across the async message boundary.

**Key operational signals & recommended alerts:**

| Signal | Why it matters | Suggested alert |
|---|---|---|
| Broadcast `Lagged` rate (per plane) | clients can't keep up; stream churn | audience‑plane lag spike ⇒ raise `CHAT_AUDIENCE_STREAM_BUFFER_SIZE` / add pods |
| Scylla `messages_by_conversation` read p99 by profile | cold‑history read pressure / cache miss | p99 > SLO ⇒ verify hot‑tail cache hit rate |
| Hot‑tail cache hit ratio | read offload health | sustained drop ⇒ Redis pressure / cap too small |
| Kafka consumer lag (`chat-visibility-consumer`) | delayed Audience‑Plane teardown | lag > threshold ⇒ broker/Redis investigation |
| DLQ produce rate (`chat.*.dlq`) | poison/retry‑exhausted events | any sustained rate ⇒ page |
| Active member vs audience subscriptions per pod | fan‑out skew / hotspotting | imbalance ⇒ rebalance shards |

---

## 🛠️ Local Development & Contribution

```bash
# Build / format / lint (this crate)
cargo build  -p chat
cargo fmt    -p chat
cargo clippy -p chat --all-targets

# Tests
cargo test   -p chat

# Whole workspace (CI gate)
cargo build  --workspace
cargo clippy --workspace
```

**Local backing services** (ScyllaDB + Redis Cluster + Kafka):

```bash
docker compose up -d scylla redis kafka      # from the repo root compose file
# then apply migrations:
for f in crates/services/chat/migrations/*.cql; do cqlsh -f "$f"; done
```

> Disk note: the workspace `target/` is large; `rm -rf target/debug/incremental` safely reclaims space (rebuildable cache) if a build fails with `No space left on device`.

---

## 🚨 Troubleshooting & Runbook (FAQ)

**1. `StreamPublic` returns `FAILED_PRECONDITION: conversation is not public`.**
Root cause: the conversation is `Private` (or was just unpublished). Audience access requires `visibility == Public`. Mitigation: confirm via `GetHistory` as a member, or `ToggleVisibility{make_public:true}` if intended. After an unpublish, the `VisibilityWorker` closes guest streams cluster‑wide — clients must stop retrying `StreamPublic`.

**2. Guests see no new messages, but members do.**
Root cause: the Audience Plane isn't fanning — usually **no active shard** in the routing registry (every audience pod's shard heartbeat expired) or `CHAT_AUDIENCE_SHARD_COUNT` mismatch between pods. Mitigation: verify `chat:{aud:<id>:<k>}` shard activation in Redis and that `CHAT_AUDIENCE_SHARD_COUNT` is uniform across the fleet; confirm pods can `SPUBLISH`/`SSUBSCRIBE` (Redis 7+ sharded pub/sub). New joiners still get history from the hot‑tail cache, so a fanning gap looks like "history works, live doesn't."

**3. History pagination misses messages or returns empty mid‑scroll.**
Root cause: a `CHAT_MESSAGE_BUCKET_HOURS` value that differs between writers and readers (bucket math diverges), or a scroll older than `MAX_BUCKET_WALK` (90 buckets) per request. Mitigation: make `CHAT_MESSAGE_BUCKET_HOURS` identical cluster‑wide and never change it after data exists; for deep history, page with the returned cursor rather than one large request. For audience readers, remember reads are floored at the public‑since watermark by design.
```
