# comment — 1-Level Threaded Comment Engine with GIF Support

## 🎯 Overview & Service Role

`services/comment` is the exclusive owner of comment lifecycle state within the core-platform super-app. It enforces **strict 1-level threading** (TikTok/Instagram style), supports **rich GIF attachments**, and emits **Kafka events** that drive the `services/engagement` atomic comment counters in real time.

**Critical boundaries (SRP):**
- **Owns:** comment persistence, threading invariants, author-gated deletion, lifecycle events.
- **Does NOT own:** likes/reactions on comments (handled by `services/engagement`), post or profile data.

**Business-impact metrics targeted:**
- Sub-5 ms P99 read latency for paginated top-level and reply feeds (ScyllaDB local quorum, TWCS partition locality).
- Zero `ALLOW FILTERING` queries — all access patterns are valid clustering-key prefix scans.
- At-least-once Kafka delivery for `comment.created` / `comment.deleted` events, consumed by the engagement service's `CommentEventConsumer` to increment/decrement its Redis and ScyllaDB counters.

---

## 📐 Architecture & Concepts

```
gRPC Client
    │
    ▼
CommentServiceHandler  (tonic)
    │   │
    │   ├── CommandBus ──► CreateCommentHandler ──► Comment::create() ──► ScyllaCommentRepository.insert()
    │   │                                                                └──► KafkaCommentEventPublisher → "comment.created"
    │   │
    │   └── CommandBus ──► DeleteCommentHandler ──► repo.has_active_replies()
    │                                           └──► Comment::delete(has_replies)
    │                                                    ├── Tombstone ──► repo.soft_delete()
    │                                                    └── Purge     ──► repo.purge()
    │                                                └──► KafkaCommentEventPublisher → "comment.deleted"
    │
    └── QueryBus ──► GetCommentHandler         → comment.comments       (point read, LCS)
                 ──► ListTopLevelHandler        → comment.comments_by_post (parent_id = nil UUID)
                 ──► ListRepliesHandler         → comment.comments_by_post (parent_id = comment_id)
```

### ScyllaDB Wide-Column Flat-Tree Layout

| Table | Partition key | Clustering keys | Purpose |
|---|---|---|---|
| `comment.comments` | `comment_id` | — | Source-of-truth point reads & mutations |
| `comment.comments_by_post` | `post_id` | `parent_id, created_at DESC, comment_id` | Feed pagination without ALLOW FILTERING |

**Nil UUID sentinel:** top-level comments are stored with `parent_id = 00000000-0000-0000-0000-000000000000` in `comments_by_post`. This is the lexicographically smallest UUID, so `WHERE post_id = ? AND parent_id = <nil>` is a valid clustering-prefix scan. Replies use their actual parent `comment_id`.

### Deletion Strategy Decision Tree

```
DeleteComment called
        │
        ▼
has_active_replies? ──Yes──► Tombstone: null body+gif, status=Deleted, keep row
        │                    → feed remains navigable for reply thread
        No
        │
        ▼
      Purge: DELETE from both tables physically
```

Both paths emit `CommentDeleted` to Kafka regardless of strategy.

### Resilience Guarantees

| Concern | Mechanism |
|---|---|
| ScyllaDB write failure on insert | Caller receives `CommentError::Storage`; idempotent retry via same `comment_id` (INSERT is last-write-wins) |
| Kafka publish failure | `CommentError::EventPublishFailed`; engagement counters lag until retry — eventual consistency |
| Feed table lagging after soft-delete | Both tables updated in same handler; window is sub-ms |
| Pagination correctness under concurrent inserts | Cursor based on `created_at DESC`; new inserts after cursor are never returned — monotonically stable pages |

---

## 🔌 Public Interfaces & API Contract

### Proto service

```protobuf
service CommentService {
  rpc CreateComment (CreateCommentRequest) returns (CreateCommentResponse);
  rpc DeleteComment (DeleteCommentRequest) returns (CommandResponse);
  rpc GetComment    (GetCommentRequest)    returns (CommentView);
  rpc ListTopLevel  (ListTopLevelRequest)  returns (ListCommentsResponse);
  rpc ListReplies   (ListRepliesRequest)   returns (ListCommentsResponse);
}
```

### Domain invariants (enforced at aggregate boundary)

| Invariant | Enforcement |
|---|---|
| Text ≤ 500 chars | `CommentBody::new()` |
| Must have text OR gif OR both | `Comment::create()` — `EmptyContent` error |
| GIF metadata must be complete | `parse_gif()` — `IncompleteGifMetadata` error |
| 1-level max nesting | `Comment::create(parent_is_top_level)` — `NestingDepthExceeded` error |
| Cannot reply to a deleted parent | `CreateCommentHandler` — `ParentDeleted` error |
| Only author may delete | `DeleteCommentHandler` — `AuthorMismatch` error |
| Deleted comments cannot be re-deleted | `Comment::delete()` — `CommentAlreadyDeleted` error |

### Error code table

| Code | Error | HTTP |
|---|---|---|
| `CMT-1001` | Comment not found | 404 |
| `CMT-1002` | Comment already deleted | 409 |
| `CMT-1003` | Author mismatch (forbidden) | 403 |
| `CMT-2001` | Nesting depth exceeded | 422 |
| `CMT-2002` | Parent comment not found | 404 |
| `CMT-2003` | Parent comment is deleted | 422 |
| `CMT-3001` | Empty content | 422 |
| `CMT-3002` | Incomplete GIF metadata | 422 |
| `CMT-4001` | Kafka publish failed | 500 |
| `CMT-9001` | Invalid comment ID | 422 |
| `CMT-9002` | Invalid post ID | 422 |
| `CMT-9003` | Invalid profile ID | 422 |
| `CMT-9004` | Domain violation | 422 |

### Kafka events published

| Topic | Key | Payload fields | Consumer |
|---|---|---|---|
| `comment.created` | `comment_id` | `comment_id, post_id, author_id, parent_id, created_at_ms` | `services/engagement` (increments comment counter) |
| `comment.deleted` | `comment_id` | `comment_id, post_id, author_id, deleted_at_ms` | `services/engagement` (decrements comment counter) |

---

## 📦 Integration & Usage

```toml
# Cargo.toml
[dependencies]
comment = { path = "crates/services/comment" }
```

### Bootstrap pattern

```rust
use comment::infrastructure::grpc::server::serve;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    telemetry::init();
    let addr = "[::1]:50054".parse()?;
    serve(addr).await
}
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_NODES` | Yes | — | Comma-separated ScyllaDB contact points (`host:port`) |
| `SCYLLA_KEYSPACE` | No | `comment` | ScyllaDB keyspace name |
| `SCYLLA_USERNAME` | No | — | ScyllaDB authentication username |
| `SCYLLA_PASSWORD` | No | — | ScyllaDB authentication password |
| `KAFKA_BOOTSTRAP_SERVERS` | Yes | — | Comma-separated Kafka broker addresses |
| `KAFKA_SECURITY_PROTOCOL` | No | `PLAINTEXT` | `PLAINTEXT` or `SASL_SSL` |
| `KAFKA_SASL_USERNAME` | No | — | Kafka SASL username (required when `SASL_SSL`) |
| `KAFKA_SASL_PASSWORD` | No | — | Kafka SASL password (required when `SASL_SSL`) |
| `GRPC_PORT` | No | `50054` | Port for the gRPC server to bind |
| `RUST_LOG` | No | `info` | Tracing filter (e.g. `comment=debug,info`) |

---

## 📈 Telemetry, Performance & Metrics

**Runtime prerequisite:** Tokio multi-thread runtime (`rt-multi-thread` feature).

### Key OTel spans emitted

| Span | Layer |
|---|---|
| `comment.create` | CQRS command handler |
| `comment.delete` | CQRS command handler |
| `scylla.insert` / `scylla.select` / `scylla.update` / `scylla.delete` | ScyllaDB `HistoryListener` |
| `kafka.publish` | Kafka producer |

### Recommended Prometheus alerts

| Metric pattern | Alert condition | Severity |
|---|---|---|
| `comment_event_publish_errors_total` | `rate > 0` for 5 min | High — engagement counters will diverge |
| `scylla_execution_errors_total{service="comment"}` | `rate > 0.1` for 2 min | High |
| `grpc_server_handling_seconds_bucket{rpc="CreateComment"}` | P99 > 500 ms | Medium |
| `grpc_server_handled_total{grpc_code="FAILED_PRECONDITION"}` | Spike > baseline | Low — possible client-side abuse |

---

## 🛠️ Local Development & Contribution

```bash
# Start infrastructure dependencies
docker compose up -d scylla kafka

# Apply CQL migrations (manual or via migration tool)
cqlsh < crates/services/comment/migrations/0001_create_keyspace.cql
cqlsh < crates/services/comment/migrations/0002_create_comments_table.cql
cqlsh < crates/services/comment/migrations/0003_create_comments_by_post_table.cql

# Build
cargo build -p comment

# Lint
cargo clippy -p comment -- -D warnings

# Format
cargo fmt -p comment

# Unit tests
cargo test -p comment
```

---

## 🚨 Troubleshooting & Runbook

### 1. Engagement comment counters are stale after comment creation

**Root cause:** Kafka publish succeeded but the engagement service's `CommentEventConsumer` is lagging or stopped.

**Mitigation:**
1. Check `engagement-comment-consumer` consumer-group lag in your Kafka console.
2. Restart the `CommentEventConsumer` background task in the engagement service.
3. The engagement service can rebuild its counter from its own ScyllaDB `post_interaction_counters` table on restart — no manual reconciliation needed.

### 2. `CMT-2001 NestingDepthExceeded` returned for a valid reply

**Root cause:** The `parent_id` passed in the request refers to a reply comment (non-nil UUID parent), not a top-level comment. The client is attempting to create a reply-to-reply.

**Mitigation:** The client must always use the original top-level `comment_id` as `parent_id`, never the `comment_id` of a reply. Verify by checking `parent_id` in `comment.comments` for the target comment — it must be `00000000-0000-0000-0000-000000000000`.

### 3. Feed table (`comments_by_post`) shows deleted content after soft-delete

**Root cause:** ScyllaDB read-your-writes consistency is not guaranteed across nodes at `LOCAL_ONE`. The `ScyllaProfileKind::Fast` profile may return a stale replica immediately after a `Strict` write.

**Mitigation:** This is expected eventual-consistency behaviour. The soft-delete propagates within milliseconds. For user-facing reads that must be consistent (e.g., author re-loading their own comment), add a short retry with `LOCAL_QUORUM` or route through the `GetComment` RPC (which reads from `comment.comments` under the `Fast` profile, which still converges faster than the feed table).
