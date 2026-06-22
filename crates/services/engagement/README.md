# engagement — Weighted Reaction Scoring Engine & Interaction Counter

## 🎯 Overview & Service Role

The **engagement** service is the real-time interaction backbone of the Super-App. It owns and operates three categories of engagement data for every published post:

- **Weighted reactions** (emoji/icon/gif) — each kind carries a configurable score weight; one active reaction per `(post_id, profile_id)` pair is enforced as an invariant.
- **High-volume counters** (views, shares) — incremented atomically in Redis and periodically flushed to ScyllaDB via a write-behind worker.
- **Comment counts** — reactively ingested from the `services/comment` Kafka event stream.

**Core business impact:**
- Sub-millisecond reaction swap with zero ScyllaDB reads on the hot path (Lua atomic swap).
- Supports millions of concurrent reactions without Paxos overhead (no ScyllaDB LWT).
- Eliminates rapid-toggle race conditions: concurrent swaps for the same `(post, profile)` pair are serialized by Redis's single-threaded Lua execution context.
- Kafka write-behind provides multi-region event streaming and process-crash durability for ledger state.

---

## 📐 Architecture & Concepts

### Data Flow

```
┌────────────────────────────────────────────────────────────────────────────┐
│                          WRITE PATH  (hot, <5 ms)                          │
│                                                                            │
│  gRPC Client ──► EngagementServiceHandler                                  │
│                       │                                                    │
│          ┌────────────┼────────────┬─────────────┐                        │
│          ▼            ▼            ▼             ▼                        │
│  UpsertReaction  RemoveReaction RecordView   RecordShare                   │
│       │               │             │             │                        │
│       ▼               ▼             ▼             ▼                        │
│  RedisScoreStore (Lua EVAL / INCR — one round-trip each)                  │
│       │               │                                                    │
│       ▼               ▼                                                    │
│  KafkaProducer  (engagement.reactions — keyed by post_id:profile_id)      │
└────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────┐
│                      WRITE-BEHIND PATH  (async)                            │
│                                                                            │
│  ReactionWriteBehindWorker                                                 │
│      Kafka consumer: engagement.reactions                                  │
│      → ScyllaDB UPSERT engagement.post_reactions (idempotent)              │
│                                                                            │
│  CounterFlushWorker  (every 5 s by default)                                │
│      DirtyPostTracker → Redis GETSET 0 (atomic snapshot)                  │
│      → ScyllaDB COUNTER UPDATE engagement.post_interaction_counters        │
│                                                                            │
│  CommentEventConsumer                                                      │
│      Kafka: comment.created / comment.deleted                              │
│      → Redis INCR/DECR engagement:comments:{post_id}                      │
│      → ScyllaDB COUNTER UPDATE engagement.post_interaction_counters        │
└────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────┐
│                          READ PATH  (queries)                              │
│                                                                            │
│  GetPostEngagement → RedisScoreStore::get_snapshot()                       │
│      HGETALL engagement:scores:{post_id}   (weighted scores per kind)     │
│      GET    engagement:views:{post_id}                                     │
│      GET    engagement:shares:{post_id}                                    │
│      GET    engagement:comments:{post_id}                                  │
│  (4 parallel GET operations, ~0.3 ms at p99)                              │
└────────────────────────────────────────────────────────────────────────────┘
```

### Redis Key Layout

| Key Pattern | Type | Purpose |
|---|---|---|
| `engagement:r:{post_id}:{profile_id}` | `HASH { kind, weight }` | Per-profile reaction state — atomic swap source |
| `engagement:scores:{post_id}` | `HASH { <kind>: <i64> }` | Real-time weighted scores — authoritative |
| `engagement:views:{post_id}` | `STRING (i64)` | Accumulated view count — periodic flush |
| `engagement:shares:{post_id}` | `STRING (i64)` | Accumulated share count — periodic flush |
| `engagement:comments:{post_id}` | `STRING (i64)` | Comment count — driven by Kafka |

### ScyllaDB Schema

```
engagement.post_reactions          — Durable ledger. PRIMARY KEY ((post_id), profile_id).
engagement.post_interaction_counters — Approximate counter table (views/shares/comments).
```

### Resilience Guarantees & High-Load Behavior

| Scenario | Behavior |
|---|---|
| Redis unavailable | gRPC commands return `503 Unavailable`. Backpressure propagates to callers. |
| ScyllaDB unavailable | Write-behind workers back off and retry. Redis state remains consistent. |
| Worker process crash | Kafka consumer group re-assigns partitions; messages re-delivered (at-least-once via `run_consumer`). Ledger UPSERT is idempotent; transient failures retry with backoff then dead-letter to `{topic}.dlq`. See the [consumer runtime standard](../../shared/transport/README.md#consumer-runtime-standard). |
| Redis restart / flush | Counter data lost for the current flush window. Reaction state lost until cold-start recovery runs against ScyllaDB ledger. |
| Rapid reaction toggle | Redis Lua serializes all swap operations for the same `(post, profile)` — no race condition possible. |
| Counter table double-count | ScyllaDB counters are approximate analytics only; Redis is authoritative. |

---

## 🔌 Public Interfaces & API Contract

### Proto service (`engagement.v1.EngagementService`)

```protobuf
service EngagementService {
    rpc UpsertReaction    (UpsertReactionRequest)    returns (CommandResponse);
    rpc RemoveReaction    (RemoveReactionRequest)     returns (CommandResponse);
    rpc RecordView        (RecordViewRequest)         returns (CommandResponse);
    rpc RecordShare       (RecordShareRequest)        returns (CommandResponse);
    rpc GetPostEngagement (GetPostEngagementRequest)  returns (PostEngagementView);
}
```

### Core port traits

```rust
// Redis-primary scoring layer
#[async_trait]
pub trait ScoreStore: Send + Sync + 'static {
    async fn atomic_upsert_reaction(
        &self, post_id: &PostId, profile_id: &ProfileId,
        new_kind: ReactionKind, new_weight: i64,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError>;

    async fn atomic_remove_reaction(
        &self, post_id: &PostId, profile_id: &ProfileId,
    ) -> Result<Option<(ReactionKind, i64)>, EngagementError>;

    async fn incr_view(&self, post_id: &PostId) -> Result<(), EngagementError>;
    async fn incr_share(&self, post_id: &PostId) -> Result<(), EngagementError>;
    async fn get_snapshot(&self, post_id: &PostId) -> Result<PostEngagementSnapshot, EngagementError>;
}

// ScyllaDB durable ledger (write-behind path only)
#[async_trait]
pub trait ReactionLedger: Send + Sync + 'static {
    async fn upsert(...) -> Result<(), EngagementError>;
    async fn remove(...) -> Result<(), EngagementError>;
    async fn scan_for_recovery(post_id: &PostId) -> Result<Vec<ReactionRow>, EngagementError>;
    async fn apply_interaction_delta(...) -> Result<(), EngagementError>;
}
```

### Error code namespace

| Range | Category |
|---|---|
| `ENG-1xxx` | Reaction state violations (not found, wrong author) |
| `ENG-2xxx` | Reaction kind / weight validation |
| `ENG-3xxx` | Kafka / event publish errors |
| `ENG-5xxx` | Worker / Lua script errors |
| `ENG-9xxx` | ID parsing / domain violations |

---

## 📦 Integration & Usage

### Cargo.toml dependency

```toml
engagement = { path = "crates/services/engagement" }
```

### Bootstrap pattern

```rust
use engagement::infrastructure::grpc::server::serve;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load ENGAGEMENT_* and SCYLLA_* and REDIS_* and KAFKA_* env vars
    let addr = "0.0.0.0:50054".parse()?;
    serve(addr).await
}
```

---

## ⚙️ Configuration & Runtime Environment

### Reaction weight matrix

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGAGEMENT_REACTION_WEIGHT_HEART` | No | `1` | Score weight for ❤️ Heart reactions |
| `ENGAGEMENT_REACTION_WEIGHT_FIRE` | No | `2` | Score weight for 🔥 Fire reactions |
| `ENGAGEMENT_REACTION_WEIGHT_ROCKET` | No | `5` | Score weight for 🚀 Rocket reactions |
| `ENGAGEMENT_REACTION_WEIGHT_CLAP` | No | `1` | Score weight for 👏 Clap reactions |
| `ENGAGEMENT_REACTION_WEIGHT_SAD` | No | `1` | Score weight for 😢 Sad reactions |

### ScyllaDB

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_CONTACT_POINTS` | **Yes** | — | Comma-separated node addresses, e.g. `127.0.0.1:9042` |
| `SCYLLA_LOCAL_DC` | **Yes** | — | Local datacenter name; must match `datacenter1` in dev |
| `SCYLLA_KEYSPACE` | No | — | Keyspace to `USE` after session bootstrap |
| `SCYLLA_USERNAME` | No | — | CQL username |
| `SCYLLA_PASSWORD` | No | — | CQL password |

### Redis

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_URL` | **Yes** | — | Redis connection URL, e.g. `redis://127.0.0.1:6379` |
| `REDIS_POOL_SIZE` | No | `8` | Connection pool size |

### Kafka

| Variable | Required | Default | Description |
|---|---|---|---|
| `KAFKA_BROKERS` | **Yes** | `localhost:9092` | Comma-separated broker list |
| `KAFKA_SECURITY_PROTOCOL` | No | `PLAINTEXT` | `PLAINTEXT` or `SASL_SSL` |
| `KAFKA_SASL_MECHANISM` | No | — | e.g. `PLAIN`, `SCRAM-SHA-256` |
| `KAFKA_SASL_USERNAME` | No | — | SASL username |
| `KAFKA_SASL_PASSWORD` | No | — | SASL password |

### Service

| Variable | Required | Default | Description |
|---|---|---|---|
| `ENGAGEMENT_GRPC_PORT` | No | `50054` | gRPC server bind port |
| `ENGAGEMENT_COUNTER_FLUSH_INTERVAL_SECS` | No | `5` | View/share flush cadence in seconds |
| `RUST_LOG` | No | `info` | Log filter (e.g. `engagement=debug,info`) |

---

## 📈 Telemetry, Performance & Metrics

### Runtime prerequisites

- Tokio multi-thread runtime (`rt-multi-thread` feature required).
- Redis AOF persistence enabled (`appendonly yes`) to survive the flush outbox.
- Kafka topic `engagement.reactions` pre-created with at least 12 partitions.
- Kafka topics `comment.created` and `comment.deleted` consumed but not owned by this service.

### Key operational metrics

| Metric | Type | Alert |
|---|---|---|
| `engagement_reaction_upsert_duration_ms` | Histogram | p99 > 10 ms → Redis latency spike |
| `engagement_reaction_upsert_total` | Counter | Sudden drop → upstream client issue |
| `engagement_view_incr_total` | Counter | Baseline health check |
| `engagement_counter_flush_lag_posts` | Gauge | > 10 000 → flush worker fallen behind |
| `engagement_write_behind_consumer_lag` | Gauge | > 50 000 → Kafka consumer group lagging |
| `engagement_redis_errors_total` | Counter | Any spike → Redis connectivity |
| `engagement_scylla_errors_total` | Counter | Any spike → ScyllaDB connectivity |

---

## 🛠️ Local Development & Contribution

### Prerequisites

```bash
docker compose up -d scylla redis kafka
```

Apply schema migrations manually:

```bash
cqlsh < migrations/0001_create_keyspace.cql
cqlsh < migrations/0002_create_post_reactions_table.cql
cqlsh < migrations/0003_create_post_interaction_counters_table.cql
```

### Build & check

```bash
# from workspace root
cargo build -p engagement
cargo clippy -p engagement -- -D warnings
cargo fmt -p engagement -- --check
```

### Test

```bash
cargo test -p engagement
```

---

## 🚨 Troubleshooting & Runbook

### 1. Reaction scores drift after Redis restart

**Root cause**: Redis was flushed or restarted without AOF persistence. The `engagement:scores:{post_id}` and `engagement:r:{post_id}:{profile_id}` hashes are lost.

**Mitigation**:
1. Enable Redis AOF (`appendonly yes`, `appendfsync everysec`) to prevent recurrence.
2. Run the cold-start recovery procedure: scan `engagement.post_reactions` in ScyllaDB, group by `(post_id, kind)`, sum weights, and rebuild Redis hashes via `HSET`.
3. The recovery script is not bundled in this service — run it as an offline maintenance job before restarting the gRPC server.

### 2. Write-behind consumer lag growing continuously

**Root cause**: ScyllaDB writes are slower than the Kafka produce rate, or the consumer group has too few members.

**Mitigation**:
1. Check `engagement_write_behind_consumer_lag` in Grafana.
2. Scale up the number of `ReactionWriteBehindWorker` instances (one per service replica suffices for typical load; each processes a dedicated partition set).
3. Verify ScyllaDB `engagement.post_reactions` table compaction is not saturating disk I/O (check `sstable_count` per node).

### 3. `ENG-5001 ScriptReturnInvalid` errors in logs

**Root cause**: The Lua swap script returned an unexpected value type. Usually caused by a Redis version incompatibility (EVAL behavior differs between Redis 6.x and 7.x regarding null returns).

**Mitigation**:
1. Verify Redis version is ≥ 7.0 (`redis-cli INFO server | grep redis_version`).
2. Check for key type corruption: `redis-cli TYPE engagement:r:{post_id}:{profile_id}` should return `hash`.
3. If key is of wrong type, delete it and let the next upsert recreate it: the Kafka outbox will still replay the reaction to ScyllaDB.
