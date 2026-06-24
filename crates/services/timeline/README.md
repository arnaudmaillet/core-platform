# timeline — Hybrid Fan-out Home Feed Aggregation Engine

## Overview & Service Role

The timeline service provides the home feed ("Following" tab) for the location-first super-app. It aggregates posts from accounts a user follows into a ranked, paginated stream using a **hybrid Fan-out-on-Write / Fan-out-on-Read** architecture that prevents the Celebrity Fan-out Problem (VIP authors with millions of followers) from saturating the write path.

**Critical business impact:**
- **Sub-millisecond hot reads:** User feeds are pre-materialized in Redis ZSETs, sorted by `published_at_ms`, paginated via opaque cursors. No scatter-gather on read for Standard/Premium authors.
- **VIP isolation:** Authors classified as `Vip` never trigger fan-out to followers' feeds on publish. Instead their posts are registered in a per-author ZSET and merged in-process at query time — bounding write amplification to O(1) per post regardless of follower count.
- **Zero post content stored:** The service stores only `(post_id, author_id, published_at_ms)` tokens. All content hydration is delegated to the client/BFF calling services/post.
- **Cold-start transparency:** When a feed has never been warmed, ScyllaDB data is returned immediately and Redis is populated asynchronously. The `is_cold` flag in the response lets the BFF show a "feed is loading" indicator.

---

## Architecture & Concepts

```
                    ┌──────────────────────────────────────────────────────────────────┐
                    │                        Kafka Cluster                             │
                    │  post.published  │  post.deleted  │  social-graph.followed/unfollowed │
                    └────────┬─────────┴───────┬────────┴──────────────┬──────────────┘
                             │                 │                       │
              ┌──────────────▼───────┐ ┌───────▼──────────┐ ┌─────────▼──────────────┐
              │ PostPublishedWorker  │ │ PostDeletedWorker │ │ FollowCreatedWorker    │
              │                      │ │                   │ │ FollowDeletedWorker    │
              │ Tier routing:        │ │ VIP: ZREM from    │ │ Created: add to        │
              │ Standard/Premium →   │ │ vip ZSET          │ │ following:set, backfill│
              │   fan-out to all     │ │ Std/Prem: purge   │ │ Std/Prem posts         │
              │   followers' ZSETs   │ │ ScyllaDB only     │ │ Deleted: remove from  │
              │ Vip → ZADD into      │ │ (Redis: eventual) │ │ following:set, prune  │
              │   timeline:vip:{}    │ │                   │ │ Std/Prem posts         │
              └──────────────────────┘ └───────────────────┘ └────────────────────────┘
                             │                 │                       │
                    ┌────────▼─────────────────▼───────────────────────▼────────────┐
                    │                     Redis (hot layer)                         │
                    │  timeline:feed:{profile_id}   ZSET  (per-follower materialized)│
                    │  timeline:vip:{author_id}     ZSET  (per-VIP-author registry) │
                    │  timeline:following:{id}      SET   (following list cache)    │
                    │  timeline:tier:{author_id}    STRING (author tier cache)      │
                    │  timeline:warm:{profile_id}   STRING (warm flag + TTL)        │
                    └─────────────────────────────────────────────────────────────┬─┘
                                                                                  │ cold-start
                    ┌─────────────────────────────────────────────────────────────▼─┐
                    │                  ScyllaDB (durable cold store)                │
                    │  timeline.feed_items_by_profile  (per-follower feed TWCS)    │
                    │  timeline.posts_by_author        (per-author reverse index)  │
                    └───────────────────────────────────────────────────────────────┘
                                                                │
                    ┌───────────────────────────────────────────▼────────────────────┐
                    │                   gRPC (GetFollowingFeed)                      │
                    │                        ↕ timeline.proto                        │
                    │                      BFF / Mobile clients                      │
                    └────────────────────────────────────────────────────────────────┘
```

### Fan-out Mode Routing

Author tier is denormalized into every `post.published` Kafka event by `services/post` (sourced from `services/profile` via `services/geo-discovery`). Timeline consumes it directly — **no synchronous tier lookup on the write path**.

| Tier | Fan-out mode | Write behaviour | Read behaviour |
|------|-------------|-----------------|----------------|
| `Standard` (0) | `Write` | Push to every follower's Redis ZSET + ScyllaDB INSERT | Serve from `timeline:feed:{profile_id}` |
| `Premium` (1)  | `Write` | Same as Standard | Same as Standard |
| `Vip` (2)      | `Read`  | ZADD into `timeline:vip:{author_id}` only | Merge at query time via `try_join_all` |

This is a **hard domain invariant** encoded in `AuthorTier::fan_out_mode()` — not a runtime config flag.

### Cold-Start Flow

1. `GetFollowingFeedQuery` checks `timeline:warm:{profile_id}` existence.
2. **Miss (first request or TTL expired):**
   - Read regular feed from `feed_items_by_profile` (ScyllaDB).
   - Read VIP slices from `posts_by_author` (ScyllaDB).
   - Return merged page with `is_cold = true`.
   - Spawn background task that reads ScyllaDB, writes to Redis ZSETs, sets warm flag.
3. **Hit:** Pipeline Redis reads for regular feed + N VIP ZSETs, merge in-process, return `is_cold = false`.

### Following Set Rebuild

On cache miss for `timeline:following:{profile_id}` (e.g. after Redis eviction):
1. Paginate `SocialGraphService.ListFollowing(profile_id)` to completion.
2. Populate `timeline:following:{profile_id}` Redis SET.
3. Per-author tier looked up from `timeline:tier:{author_id}` to split VIP vs regular.
4. Cache miss on tier → conservatively route to `Standard` (no blocking; tier updated on next `post.published`).

---

## Data Model

### Redis Keys

| Key pattern | Type | Purpose | Cap / TTL |
|-------------|------|---------|-----------|
| `timeline:feed:{profile_id}` | ZSET | Materialized feed for Standard/Premium follows | Cap: `FEED_CAP` (default 500); no TTL |
| `timeline:vip:{author_id}` | ZSET | Per-VIP recent post registry | Cap: `VIP_REGISTRY_CAP` (default 200); TTL: `VIP_REGISTRY_TTL_SECS` |
| `timeline:following:{profile_id}` | SET | Following list cache for read-path routing | No TTL (rebuilt on miss) |
| `timeline:tier:{author_id}` | STRING | Author tier cache | TTL: `TIER_CACHE_TTL_SECS` (default 3600s) |
| `timeline:warm:{profile_id}` | STRING | Warm flag — existence signals Redis is populated | TTL: `WARM_TTL_SECS` (default 86400s) |

All ZSET entries are scored by `published_at_ms`. Feed ZSET members are encoded as `"{post_id}:{author_id}"` so the BFF can identify the author without a secondary lookup.

### ScyllaDB Tables

#### `timeline.feed_items_by_profile`

Partition: `profile_id (uuid)` — per-follower feed.
Clustering: `(published_at DESC, post_id ASC)` — recency-first, UUID v7 tie-break.

```cql
CREATE TABLE timeline.feed_items_by_profile (
    profile_id  uuid,
    published_at timestamp,
    post_id     uuid,
    author_id   uuid,
    PRIMARY KEY (profile_id, published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = { 'class': 'TimeWindowCompactionStrategy',
                     'compaction_window_unit': 'DAYS',
                     'compaction_window_size': 30 }
  AND default_time_to_live = 2592000;
```

Role: Cold-start source for regular feeds. `author_id` column enables unfollow-prune scans.

#### `timeline.posts_by_author`

Partition: `author_id (uuid)` — per-author reverse index.
Clustering: `(published_at DESC, post_id ASC)`.

```cql
CREATE TABLE timeline.posts_by_author (
    author_id   uuid,
    published_at timestamp,
    post_id     uuid,
    author_tier tinyint,
    PRIMARY KEY (author_id, published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = { 'class': 'TimeWindowCompactionStrategy', ... }
  AND default_time_to_live = 2592000;
```

Dual role:
1. **VIP cold-start source** — when `timeline:vip:{author_id}` is absent.
2. **Follow-backfill source** — `FollowCreatedWorker` queries `posts_by_author` to retroactively populate a new follower's Redis feed.

---

## Kafka Events Consumed

| Topic | Worker | Event fields | Action |
|-------|--------|-------------|--------|
| `post.published` | `PostPublishedWorker` | `post_id`, `profile_id` (alias `author_id`), `author_tier`, `published_at_ms` | Fan-out to followers (Std/Prem) or register in VIP ZSET |
| `post.deleted` | `PostDeletedWorker` | `post_id`, `profile_id`, `author_tier`, `published_at_ms` | Remove from VIP ZSET or prune from ScyllaDB |
| `social-graph.followed` | `FollowCreatedWorker` | `actor_id` (follower), `target_id` (followee) | Backfill recent posts (Std/Prem), update following set |
| `social-graph.unfollowed` | `FollowDeletedWorker` | `actor_id`, `target_id` | Prune posts from feed (Std/Prem), update following set |

All workers run on the shared `run_consumer` at-least-once standard: manual offset commit after success, transient failures retried with bounded backoff then dead-lettered to `{topic}.dlq`, poison records dead-lettered immediately. All downstream writes are idempotent (ZADD is idempotent; ScyllaDB upserts via `INSERT`). See the [consumer runtime standard](../../shared/transport/README.md#consumer-runtime-standard).

---

## gRPC API

**Package:** `timeline.v1`

### `TimelineService.GetFollowingFeed`

```protobuf
rpc GetFollowingFeed(GetFollowingFeedRequest) returns (GetFollowingFeedResponse);

message GetFollowingFeedRequest {
    string profile_id  = 1;  // UUID of the requesting user
    int32  limit       = 2;  // Page size (clamped to MAX_PAGE_SIZE)
    string page_token  = 3;  // Opaque cursor from previous response
}

message GetFollowingFeedResponse {
    repeated FeedItem items          = 1;
    string            next_page_token = 2;  // Empty when no more pages
    bool              is_cold         = 3;  // True when served from ScyllaDB
}

message FeedItem {
    string post_id         = 1;
    string author_id       = 2;
    int64  published_at_ms = 3;  // Unix epoch milliseconds
}
```

**Cursor format:** `base64url("{published_at_ms}:{post_id_hyphenated}")` — opaque to clients; decode only server-side.

---

## Configuration (Environment Variables)

| Variable | Default | Description |
|----------|---------|-------------|
| `TIMELINE_FEED_CAP` | `500` | Max entries per follower's Redis ZSET |
| `TIMELINE_VIP_REGISTRY_CAP` | `200` | Max entries per VIP author's ZSET |
| `TIMELINE_BACKFILL_LIMIT` | `100` | Max posts to backfill on follow |
| `TIMELINE_WARM_TTL_SECS` | `86400` | Warm flag TTL (24 h) |
| `TIMELINE_TIER_CACHE_TTL_SECS` | `3600` | Author tier cache TTL (1 h) |
| `TIMELINE_VIP_REGISTRY_TTL_SECS` | `604800` | VIP ZSET TTL (7 days) |
| `TIMELINE_MAX_PAGE_SIZE` | `50` | Maximum allowed page size for GetFollowingFeed |
| `TIMELINE_MAX_VIP_MERGE_SOURCES` | `50` | Max VIP ZSETs merged per request |
| `TIMELINE_SOCIAL_GRAPH_PAGE_SIZE` | `500` | Pagination size for social-graph ListFollowers/ListFollowing |
| `TIMELINE_SOCIAL_GRAPH_ENDPOINT` | `http://social-graph:50051` | gRPC endpoint for services/social-graph |
| `TIMELINE_KAFKA_GROUP_POST_PUBLISHED` | `timeline-post-published` | Consumer group ID |
| `TIMELINE_KAFKA_GROUP_POST_DELETED` | `timeline-post-deleted` | Consumer group ID |
| `TIMELINE_KAFKA_GROUP_SG_FOLLOWED` | `timeline-sg-followed` | Consumer group ID |
| `TIMELINE_KAFKA_GROUP_SG_UNFOLLOWED` | `timeline-sg-unfollowed` | Consumer group ID |

Standard ScyllaDB, Redis, and Kafka connection variables from `crates/shared/storage/` apply.

---

## Error Codes

| Code | Variant | HTTP | Description |
|------|---------|------|-------------|
| `TML-1001` | `FeedNotFound` | 404 | No feed found for profile |
| `TML-2001` | `FanOutFailed` | 500 | Write fan-out dispatch failure |
| `TML-2002` | `VipRegistryWriteFailed` | 500 | VIP ZSET write failure |
| `TML-3001` | `SocialGraphClientError` | 500 | gRPC call to social-graph failed (retryable) |
| `TML-3002` | `SocialGraphInvalidId` | 500 | Invalid profile ID in social-graph response |
| `TML-4001` | `ColdStartFailed` | 500 | ScyllaDB cold-start read failure |
| `TML-5001` | `ScriptReturnInvalid` | 500 | Lua script returned unexpected format |
| `TML-5002` | `BackfillFailed` | 500 | Follow-backfill write failure |
| `TML-6001` | `InvalidPageToken` | 422 | Malformed or expired pagination cursor |
| `TML-9001` | `InvalidPostId` | 422 | Post UUID parse failure |
| `TML-9002` | `InvalidProfileId` | 422 | Profile UUID parse failure |
| `TML-9003` | `InvalidAuthorId` | 422 | Author UUID parse failure |
| `TML-9004` | `DomainViolation` | 422 | Generic domain constraint violation |

Shared storage errors inherit their codes from `crates/shared/storage/postgres` and `crates/shared/storage/scylla`.

---

## Module Structure

```
src/
├── lib.rs
├── config/mod.rs              # TimelineConfig — from_env()
├── error.rs                   # TimelineError + AppError impl
├── domain/
│   ├── aggregate/
│   │   └── feed_entry.rs      # FeedEntry { post_id, author_id, published_at_ms }
│   └── value_object/
│       ├── author_id.rs       # AuthorId(Uuid)
│       ├── author_tier.rs     # AuthorTier enum + FanOutMode
│       ├── cursor.rs          # FeedCursor — base64url encode/decode
│       ├── post_id.rs         # PostId(Uuid)
│       └── profile_id.rs      # ProfileId(Uuid)
├── application/
│   ├── command/
│   │   ├── ingest_post_published.rs  # IngestPostPublishedCommand + Handler
│   │   ├── remove_post.rs            # RemovePostCommand + Handler
│   │   ├── backfill_follow.rs        # BackfillFollowCommand + Handler
│   │   └── prune_follow.rs           # PruneFollowCommand + Handler
│   ├── query/
│   │   └── get_following_feed.rs     # GetFollowingFeedQuery + Handler
│   └── port/
│       ├── feed_store.rs             # FeedStore trait (Redis hot layer)
│       ├── vip_registry.rs           # VipRegistry trait (VIP ZSET)
│       ├── tier_cache.rs             # TierCache trait (author tier + warm flag)
│       ├── following_store.rs        # FollowingStore trait (following set)
│       ├── feed_repository.rs        # FeedRepository trait (ScyllaDB cold layer)
│       ├── author_post_repository.rs # AuthorPostRepository trait
│       └── social_graph_client.rs    # SocialGraphClient trait (gRPC)
└── infrastructure/
    ├── cache/
    │   ├── redis_feed_store.rs       # RedisFeedStore (Lua ZADD+cap+prefix-remove)
    │   ├── redis_vip_registry.rs     # RedisVipRegistry (Lua ZADD+cap+TTL)
    │   ├── redis_tier_cache.rs       # RedisTierCache (GET/SET with EX)
    │   └── redis_following_store.rs  # RedisFollowingStore (SADD/SREM/SMEMBERS)
    ├── persistence/
    │   ├── model/
    │   │   ├── feed_item_row.rs      # ScyllaDB row → FeedEntry deserialization
    │   │   └── author_post_row.rs    # ScyllaDB row → FeedEntry deserialization
    │   ├── scylla_feed_repository.rs
    │   └── scylla_author_post_repository.rs
    ├── client/
    │   └── social_graph_grpc_client.rs  # SocialGraphGrpcClient (paginated gRPC)
    ├── grpc/
    │   ├── handler/
    │   │   └── timeline_handler.rs   # TimelineServiceHandler<QB>
    │   └── server.rs                 # serve() — wires all deps + spawns workers
    └── worker/
        ├── post_published_worker.rs
        ├── post_deleted_worker.rs
        ├── follow_created_worker.rs
        └── follow_deleted_worker.rs
```

---

## Local Development

```bash
# Run ScyllaDB + Redis + Kafka via Docker
docker compose up -d scylladb redis kafka

# Apply CQL migrations
cqlsh < migrations/0001_create_keyspace.cql
cqlsh < migrations/0002_create_feed_items_by_profile_table.cql
cqlsh < migrations/0003_create_posts_by_author_table.cql

# Type-check
cargo check -p timeline

# Build
cargo build -p timeline
```

---

## 🚀 Deployment

Library-only: implements [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `timeline::service::TimelineService`. `build` maps `TimelineConfig` →
`AppConfig`, constructs the social-graph gRPC client (`SocialGraphGrpcClient` over
a lazily-connected channel, so timeline boots even if social-graph isn't yet
reachable), assembles the cache/persistence adapters and CQRS buses, and spawns
the ingestion workers; `register` adds the gRPC + reflection services (the surface
is query-only — writes arrive via Kafka); `health_probes` checks Scylla/Redis. The
deployable binary is `crates/apps/timeline-server`:

```rust
use std::net::SocketAddr;
use timeline::service::TimelineService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("TIMELINE_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50060".to_owned())
        .parse()?;
    service_runtime::serve::<TimelineService>(addr).await
}
```
