# notification — Semantic Event Ingestion, Activity Feed, and Real-Time Push Engine

## 🎯 Overview & Service Role

The notification service closes the user feedback loop for the location-first super-app. It ingests semantic business events from Kafka (`engagement.reactions`, `comment.created`, `post.published`), persists durable per-profile activity records to ScyllaDB, and dispatches real-time pushes to active mobile clients via a gRPC server-streaming channel.

**Critical business impact:**
- **Activity Feed (bell icon):** O(1) paginated read with cursor-based pagination — no ALLOW FILTERING anywhere.
- **Unread badge count:** Sub-millisecond Redis read path with ScyllaDB counter as durable fallback.
- **Fan-out protection:** Layered write-collapse pipeline prevents celebrity partitions from saturating under 10k+ reactions/second.
- **SRP boundary:** Stores semantic relation IDs only — no localized strings, no profile handles, no post content. UI hydration is delegated to the client/BFF.

---

## 📐 Architecture & Concepts

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    Kafka Cluster                            │
                    │  engagement.reactions  │  comment.created  │  post.published │
                    └─────────┬─────────────┴──────────┬─────────┴────────┬───────┘
                              │                         │                  │
              ┌───────────────▼────────┐  ┌────────────▼──────┐  ┌───────▼──────────┐
              │ ReactionNotification   │  │ CommentNotification│  │ MentionNotification│
              │ Worker                 │  │ Worker             │  │ Worker             │
              │                        │  │                    │  │                    │
              │ Layer 1: in-batch      │  │ Cache comment      │  │ Cache post author  │
              │ HashMap collapse       │  │ author → Redis     │  │ → Redis            │
              │                        │  │                    │  │ Parse @mentions    │
              │ Layer 2: Redis cross-  │  │ Block gate + self  │  │ from caption       │
              │ batch window (hot      │  │ guard              │  │                    │
              │ subjects only)         │  └─────────┬──────────┘  └────────┬───────────┘
              │                        │            │                       │
              │ Layer 3: hourly cap    │            │                       │
              └──────────┬─────────────┘            │                       │
                         │                          │                       │
              ┌──────────▼──────────────────────────▼───────────────────────▼──────────┐
              │                   CollapseFlushWorker                                  │
              │  Polls notification:window_schedule ZSET every 30 s                    │
              │  Drains settled Redis collapse windows → single ScyllaDB row           │
              └──────────────────────────────────┬───────────────────────────────────  ┘
                                                 │
              ┌──────────────────────────────────▼───────────────────────────────────┐
              │             ScyllaDB: notification.notifications_by_profile           │
              │  TWCS 7-day windows │ 90-day TTL │ Partition: target_profile_id      │
              │  Cluster: (created_at DESC, notification_id ASC)                     │
              └──────────────────────────────────┬───────────────────────────────────┘
                                                 │
              ┌──────────────────────────────────▼───────────────────────────────────┐
              │           gRPC Server  (NotificationService)                         │
              │  ListNotifications  │  GetUnreadCount  │  MarkRead  │  MarkAllRead   │
              │  StreamNotifications (server-streaming, tokio::broadcast per profile) │
              └──────────────────────────────────────────────────────────────────────┘
```

### Resilience Guarantees & High-Load Behavior

| Scenario | Behavior |
|---|---|
| **Celebrity fan-out (10k reactions/s)** | Layer 1 in-batch HashMap collapse (free, always on). Layer 2 Redis cross-batch 30-second window for subjects with heat > 100. Layer 3 hourly cap of 3 notifications per subject. |
| **Redis unavailable** | Workers log a warning and proceed without the block/heat checks. ScyllaDB writes continue. The unread counter accumulates inconsistency until Redis recovers (ScyllaDB counter is the durability anchor). |
| **ScyllaDB unavailable** | Worker goroutines log errors and continue consuming (Kafka offsets NOT committed until writes succeed in a future refactor — see Runbook). Stream pushes are best-effort. |
| **Kafka consumer lag** | Auto-offset-commit with earliest reset. Workers restart with 5s back-off on error. Horizontal scaling: add consumer replicas up to the partition count of each topic. |
| **gRPC stream slow client** | `tokio::sync::broadcast` drops old messages (capacity = `NOTIFICATION_STREAM_BUFFER_SIZE`). Client receives `RecvError::Lagged` → service terminates stream with `Status::DataLoss`. Client reconnects and re-polls `ListNotifications`. |
| **CollapseFlushWorker crash** | Redis TTL (window TTL + 10s grace) naturally expires the key. At worst one collapse window of notifications is lost. The schedule ZSET member is NOT removed on crash, so the next worker startup will attempt a re-flush (DRAIN returns 0, no-op). |

---

## 🔌 Public Interfaces & API Contract

### Proto Service

```protobuf
service NotificationService {
  rpc ListNotifications     (ListNotificationsRequest)   returns (ListNotificationsResponse);
  rpc GetUnreadCount        (GetUnreadCountRequest)      returns (GetUnreadCountResponse);
  rpc MarkRead              (MarkReadRequest)            returns (CommandResponse);
  rpc MarkAllRead           (MarkAllReadRequest)         returns (CommandResponse);
  rpc StreamNotifications   (StreamNotificationsRequest) returns (stream StreamNotificationsResponse);
}
```

### Key Invariants

- `NotificationView` contains **only UUIDs and enum integers**. No profile handles, avatars, or content text. The client hydrates those from profile and content services on demand.
- `MarkReadRequest` requires both `notification_id` AND `created_at_ms` — ScyllaDB needs the full `(created_at, notification_id)` clustering key for a point UPDATE.
- `ListNotificationsResponse.read_horizon_ms` is set by `MarkAllRead`. The client renders all notifications with `created_at_ms <= read_horizon_ms` as read, regardless of the individual `is_read` flag.

### Port Traits

```rust
// NotificationRepository — ScyllaDB
pub trait NotificationRepository: Send + Sync + 'static {
    async fn insert(&self, notification: &Notification) -> Result<(), NotificationError>;
    async fn list_paginated(&self, profile_id, limit, cursor) -> Result<(Vec<NotificationSummary>, Option<String>), NotificationError>;
    async fn mark_read(&self, profile_id, notification_id, created_at_ms) -> Result<bool, NotificationError>;
    async fn increment_counter(&self, profile_id) -> Result<(), NotificationError>;
    async fn decrement_counter(&self, profile_id) -> Result<(), NotificationError>;
    async fn reset_counter(&self, profile_id)     -> Result<(), NotificationError>;
    async fn read_counter(&self, profile_id)      -> Result<i64, NotificationError>;
}

// UnreadCounter — Redis L1 + ScyllaDB L2
pub trait UnreadCounter: Send + Sync + 'static {
    async fn increment(&self, profile_id) -> Result<(), NotificationError>;
    async fn decrement(&self, profile_id) -> Result<(), NotificationError>;
    async fn reset(&self, profile_id)     -> Result<(), NotificationError>;
    async fn get(&self, profile_id)       -> Result<i64, NotificationError>;
    async fn set_read_horizon(&self, profile_id, horizon_ms) -> Result<(), NotificationError>;
    async fn get_read_horizon(&self, profile_id) -> Result<i64, NotificationError>;
}

// BlockCache — social-graph gate
pub trait BlockCache: Send + Sync + 'static {
    async fn is_blocked(&self, sender, target) -> Result<bool, NotificationError>;
}

// StreamRegistry — real-time push surface
pub trait StreamRegistry: Send + Sync + 'static {
    fn subscribe(&self, profile_id) -> broadcast::Receiver<Arc<NotificationPayload>>;
    fn broadcast(&self, profile_id, payload: Arc<NotificationPayload>);
}
```

---

## 📦 Integration & Usage

```toml
# Cargo.toml (service binary)
[dependencies]
notification = { path = "crates/services/notification" }
```

### Bootstrap

```rust
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init_from_env()?;

    let addr: SocketAddr = std::env::var("NOTIFICATION_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50056".to_owned())
        .parse()?;

    notification::infrastructure::grpc::server::serve(addr).await
}
```

### CQL Migrations (run in order before first boot)

```bash
cqlsh -f migrations/001_keyspace.cql
cqlsh -f migrations/002_notifications_by_profile.cql
cqlsh -f migrations/003_notification_unread_counters.cql
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_HOSTS` | Yes | — | Comma-separated ScyllaDB contact points (`host:port`). |
| `SCYLLA_KEYSPACE` | No | `notification` | Default keyspace. |
| `REDIS_URL` | Yes | — | Redis connection URL (`redis://host:port`). |
| `KAFKA_BROKERS` | Yes | — | Comma-separated Kafka broker addresses. |
| `NOTIFICATION_GRPC_ADDR` | No | `0.0.0.0:50056` | gRPC server bind address. |
| `NOTIFICATION_HOT_SUBJECT_THRESHOLD` | No | `100` | Reactions per 5-minute window to classify a subject as hot and activate Redis cross-batch collapse. |
| `NOTIFICATION_COLLAPSE_WINDOW_SECS` | No | `30` | TTL of a Redis cross-batch collapse window in seconds. |
| `NOTIFICATION_COLLAPSE_FLUSH_INTERVAL_SECS` | No | `30` | How often `CollapseFlushWorker` polls for settled windows. |
| `NOTIFICATION_MAX_PER_SUBJECT_PER_HOUR` | No | `3` | Maximum notifications delivered per `(target, subject, kind)` tuple per hour. |
| `NOTIFICATION_UNREAD_CAP` | No | `99` | Maximum unread badge value stored in Redis (displays as "99+"). |
| `NOTIFICATION_POST_AUTHOR_CACHE_TTL_SECS` | No | `604800` | TTL for `notification:pa:{post_id}` Redis entries (7 days). |
| `NOTIFICATION_COMMENT_AUTHOR_CACHE_TTL_SECS` | No | `259200` | TTL for `notification:ca:{comment_id}` Redis entries (3 days). |
| `NOTIFICATION_BLOCK_CACHE_TTL_SECS` | No | `300` | TTL for point-cache block-check entries (5 minutes). |
| `NOTIFICATION_MAX_PAGE_SIZE` | No | `50` | Server-side cap on `ListNotifications` page size. |
| `NOTIFICATION_STREAM_BUFFER_SIZE` | No | `256` | `tokio::broadcast` channel capacity per active streaming profile. |
| `NOTIFICATION_MAX_SAMPLE_SENDERS` | No | `5` | Maximum distinct senders stored per collapse bucket. |

---

## 📈 Telemetry, Performance & Metrics

### Runtime Prerequisites

- Tokio multi-thread runtime (`#[tokio::main]` with default thread count = CPU cores).
- ScyllaDB 5.x+ with `datacenter1` replication factor 3.
- Redis 7.x+ (Lua scripting, keyspace notifications optional).
- Kafka 3.x+ with topics pre-created:
  - `engagement.reactions` (partitioned by `{post_id}:{profile_id}`)
  - `comment.created` / `comment.deleted` (partitioned by `comment_id`)
  - `post.published` (partitioned by `post_id`)

### Key OTel Metrics (Prometheus labels)

| Metric | Type | Description |
|---|---|---|
| `notification_written_total` | Counter | Notifications persisted to ScyllaDB. Label: `kind`. |
| `notification_suppressed_total` | Counter | Notifications dropped. Labels: `reason` (`blocked`, `self`, `capped`, `cache_miss`). |
| `notification_collapse_batch_size` | Histogram | Events collapsed per batch flush. |
| `notification_collapse_window_count` | Counter | Cross-batch Redis windows flushed by `CollapseFlushWorker`. |
| `notification_unread_cache_hit_total` | Counter | Redis hits on unread counter. |
| `notification_unread_cache_miss_total` | Counter | Falls back to ScyllaDB counter read. |
| `notification_stream_broadcast_total` | Counter | Payloads broadcast to active gRPC streams. |
| `notification_stream_lagged_total` | Counter | Streams terminated due to receiver lag. |

### Recommended Alerts

```yaml
- alert: NotificationWriteErrorRate
  expr: rate(notification_suppressed_total{reason="write_error"}[5m]) > 0.01
  severity: critical

- alert: CollapseFlushWorkerStopped
  expr: increase(notification_collapse_window_count[5m]) == 0
        and increase(notification_written_total[5m]) > 100
  severity: warning

- alert: KafkaConsumerLag
  expr: kafka_consumer_group_lag{group=~"notification-.*"} > 10000
  severity: warning
```

---

## 🛠️ Local Development & Contribution

### Prerequisites

```bash
docker compose up -d scylla redis kafka
```

### Build & Check

```bash
# Build
cargo build -p notification

# Format
cargo fmt -p notification

# Lint
cargo clippy -p notification -- -D warnings

# Run CQL migrations (requires cqlsh in PATH)
cqlsh -f crates/services/notification/migrations/001_keyspace.cql
cqlsh -f crates/services/notification/migrations/002_notifications_by_profile.cql
cqlsh -f crates/services/notification/migrations/003_notification_unread_counters.cql

# Start service
NOTIFICATION_GRPC_ADDR=0.0.0.0:50056 \
SCYLLA_HOSTS=127.0.0.1:9042 \
REDIS_URL=redis://127.0.0.1:6379 \
KAFKA_BROKERS=127.0.0.1:9092 \
  cargo run -p notification
```

### Sending a test gRPC call

```bash
# List notifications for a profile
grpcurl -plaintext \
  -d '{"profile_id": "018f..."}' \
  127.0.0.1:50056 \
  notification.v1.NotificationService/ListNotifications

# Open a streaming subscription
grpcurl -plaintext \
  -d '{"profile_id": "018f..."}' \
  127.0.0.1:50056 \
  notification.v1.NotificationService/StreamNotifications
```

---

## 🚨 Troubleshooting & Runbook

### 1. Post author cache miss — reaction notifications not appearing

**Symptom:** `NTF-6001` in logs; reaction notifications are silently dropped for a post.

**Root cause:** `ReactionNotificationWorker` looks up `notification:pa:{post_id}` before writing a REACTION notification. This key is populated by `MentionNotificationWorker` when it processes `post.published`. If the mention worker has consumer lag or the post was published before the notification service was deployed, the key is absent.

**Mitigation:**
1. Check `MentionNotificationWorker` consumer lag: `kafka-consumer-groups.sh --describe --group notification-mention-consumer`.
2. If lag is high, restart the mention worker consumer group with `auto.offset.reset=earliest` to replay.
3. For immediate recovery, manually SET the key in Redis:
   ```bash
   redis-cli SET "notification:pa:{post_id}" "{author_profile_id}" EX 604800
   ```

---

### 2. Unread count badge out of sync

**Symptom:** The Redis unread counter shows a different value than what the user sees after marking all as read.

**Root cause:** Redis was evicted (no persistence) or the `MarkAllRead` command failed after resetting Redis but before deleting the ScyllaDB counter row.

**Mitigation:**
1. Query the ScyllaDB counter directly:
   ```cql
   SELECT unread_count FROM notification.notification_unread_counters
   WHERE target_profile_id = <uuid>;
   ```
2. Rehydrate Redis from ScyllaDB (the `UnreadCounter::get` method does this automatically on cache miss):
   ```bash
   redis-cli DEL "notification:unread:{profile_id}"
   # Next GetUnreadCount call will repopulate from ScyllaDB
   ```

---

### 3. CollapseFlushWorker not flushing celebrity notifications

**Symptom:** `notification:window_schedule` ZSET has growing membership; `CollapseFlushWorker` writes are zero.

**Root cause:** The worker lost its Tokio task (panic) or `zrangebyscore` is failing due to a Redis connection issue.

**Mitigation:**
1. Check service logs for `CollapseFlushWorker` panic messages.
2. Verify Redis connectivity: `redis-cli ping`.
3. Inspect the schedule ZSET: `redis-cli ZRANGEBYSCORE notification:window_schedule -inf +inf WITHSCORES LIMIT 0 10`.
4. Manually drain a stuck window using the `DRAIN_WINDOW_SCRIPT` via `redis-cli EVAL` if the worker cannot recover within 5 minutes. The windows have a natural TTL (`collapse_window_secs + 10`) so they self-expire without double-writes.
