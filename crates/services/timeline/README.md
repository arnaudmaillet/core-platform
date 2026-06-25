# `timeline` — Hybrid fan-out home feed that keeps celebrities off the write path

> **Service Card**
>
> | | |
> |---|---|
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |
> | **On-call / escalation** | `<TODO: oncall-rotation>` → `<TODO: escalation-policy>` |
> | **Tier** | **TIER-1** — user-facing "Following" feed; derived, cold-start transparent |
> | **Deployable** | `crates/apps/timeline-server` (library crate: `crates/services/timeline`) |
> | **Datastores** | Redis (materialized feeds + VIP registries) · ScyllaDB keyspace `timeline` (durable cold store) |
> | **Async** | publishes nothing · consumes `post.published` / `post.deleted` / `social-graph.followed` / `.unfollowed` |
> | **Upstream callers** | `<TODO: BFF / mobile>`; calls `social-graph` (gRPC) |
> | **Downstream deps** | Redis, ScyllaDB, Kafka, `social-graph` |
> | **SLO** | hot-read sub-ms (Redis ZSET) · VIP write amplification O(1)/post |

---

## 🎯 Overview & Service Role

`timeline` provides the home feed (the "Following" tab). It aggregates posts from followed accounts
into a ranked, paginated stream using a **hybrid fan-out-on-write / fan-out-on-read** architecture.

The hard problem it solves is the **Celebrity Fan-out Problem**: a VIP author with millions of
followers would, under pure fan-out-on-write, generate millions of feed writes per post. It resolves
this by **tier-routing on the author**: Standard/Premium authors fan out to followers' Redis ZSETs;
VIP authors never fan out — their posts land in a per-author ZSET that is merged in-process at query
time, bounding write amplification to O(1) per post regardless of follower count.

**Core objectives:** sub-ms hot reads (pre-materialized Redis ZSETs, opaque cursors); VIP write
isolation; zero post content stored (only `(post_id, author_id, published_at_ms)` tokens — hydration is
the client/BFF's job); cold-start transparency (Scylla served immediately, Redis warmed async,
`is_cold` flag tells the BFF to show "loading").

---

## 📐 Architecture & Concepts

The gRPC surface is **query-only** — all writes arrive via Kafka workers.

```
Kafka: post.published │ post.deleted │ social-graph.followed/unfollowed
   ▼                    ▼                ▼
PostPublishedWorker  PostDeletedWorker  Follow{Created,Deleted}Worker
 (Std/Prem → fan-out  (VIP → ZREM;       (Created → add to following set,
  to followers' ZSETs;  Std/Prem → Scylla  backfill Std/Prem posts;
  VIP → ZADD vip:{})    purge)             Deleted → prune)
   └─────────────────────┬──────────────────────┘
                         ▼
   Redis: timeline:feed:{profile}  ZSET (per-follower) · timeline:vip:{author} ZSET
          timeline:following:{id}  SET · timeline:tier:{author} · timeline:warm:{profile}
                         ▼ cold-start
   ScyllaDB: timeline.feed_items_by_profile (TWCS) · timeline.posts_by_author (reverse index)
                         ▼
   gRPC TimelineService.GetFollowingFeed ─► BFF / mobile
```

**Fan-out routing** (a hard domain invariant in `AuthorTier::fan_out_mode()`, **not** a config flag):

| Tier | Mode | Write | Read |
|---|---|---|---|
| `Standard` (0) | `Write` | push to every follower ZSET + Scylla INSERT | serve `timeline:feed:{profile}` |
| `Premium` (1) | `Write` | same as Standard | same |
| `Vip` (2) | `Read` | ZADD `timeline:vip:{author}` only | merge at query time (`try_join_all`) |

Author tier is denormalized into every `post.published` event — **no synchronous tier lookup on the
write path**. ZSET members encode `"{post_id}:{author_id}"` so the BFF identifies the author without a
secondary lookup.

> **Invariants:** VIP authors never fan out (write amplification O(1)/post); cold-start returns Scylla
> data with `is_cold=true` and warms Redis async; following-set rebuild on Redis miss paginates
> `SocialGraphService.ListFollowing` and conservatively routes unknown tiers to `Standard`.

---

## 📊 Service Level Objectives (SLO)

| SLI | Objective | Window | Measured by |
|---|---|---|---|
| `GetFollowingFeed` p99 — warm (Redis) | `< <TODO> ms` | 1h | gRPC histogram |
| Cold-start fallback p99 (Scylla) | `< <TODO> ms` | 1h | Scylla read histogram |
| Fan-out ingest lag (`post.published`) | `< <TODO> s` | live | consumer-group lag |
| VIP write amplification | O(1) per post | — | invariant (`fan_out_mode`) |

**Error budget:** `<TODO>`. **On burn:** `<TODO>`.

---

## 🔗 Dependencies & Blast Radius

**Downstream:**

| Dependency | Purpose | If down → | Degradation |
|---|---|---|---|
| Redis | hot feed + VIP registries | warm reads fail | **Soft** — cold-start path serves from Scylla |
| ScyllaDB (`timeline`) | durable cold store | cold-start + ingest fail | **Hard** for cold reads; ingest retries |
| Kafka | fan-out ingest | feed stops updating | **Soft** — existing feed served |
| `social-graph` (gRPC) | following-set rebuild | rebuild on Redis miss fails | **Soft** — boots lazily; `TML-3001` retryable |

**Upstream (blast radius):**

| Caller | Uses | Impact if `timeline` is down |
|---|---|---|
| `<TODO: BFF / mobile>` | `GetFollowingFeed` | the Following home feed stops loading |

> **Critical path?** Yes for the home-feed surface; it is a derived read-model, so an outage degrades
> the feed but not posting/social actions.

---

## 🔌 Public Interfaces & API Contract

### gRPC — `timeline.v1.TimelineService`

```protobuf
rpc GetFollowingFeed(GetFollowingFeedRequest) returns (GetFollowingFeedResponse);

message GetFollowingFeedRequest  { string profile_id=1; int32 limit=2; string page_token=3; }
message GetFollowingFeedResponse { repeated FeedItem items=1; string next_page_token=2; bool is_cold=3; }
message FeedItem { string post_id=1; string author_id=2; int64 published_at_ms=3; }
```

> **Wire contract:** the cursor is `base64url("{published_at_ms}:{post_id_hyphenated}")` — opaque to
> clients, decoded server-side only. `limit` is clamped to `TIMELINE_MAX_PAGE_SIZE`. `is_cold=true` means
> the page was served from ScyllaDB while Redis warms asynchronously.

### Rust ports (hexagonal contract)

```rust
pub trait FeedStore: Send + Sync { /* Redis hot ZSET: add/cap/prefix-remove/range */ }
pub trait VipRegistry: Send + Sync { /* per-VIP ZSET (ZADD+cap+TTL) */ }
pub trait TierCache: Send + Sync { /* author tier + warm flag */ }
pub trait FollowingStore: Send + Sync { /* following set (SADD/SREM/SMEMBERS) */ }
pub trait FeedRepository / AuthorPostRepository: Send + Sync { /* ScyllaDB cold layer */ }
pub trait SocialGraphClient: Send + Sync { /* paginated gRPC to social-graph */ }
```

### Error contract (`TML-xxxx`)

| Code | Variant | HTTP |
|---|---|---|
| TML-1001 | `FeedNotFound` | 404 |
| TML-2001/2002 | `FanOutFailed` / `VipRegistryWriteFailed` | 500 |
| TML-3001/3002 | `SocialGraphClientError` (retryable) / `SocialGraphInvalidId` | 500 |
| TML-4001 | `ColdStartFailed` | 500 |
| TML-5001/5002 | `ScriptReturnInvalid` / `BackfillFailed` | 500 |
| TML-6001 | `InvalidPageToken` | 422 |
| TML-9001..9004 | invalid ids / domain violation | 422 |

---

## 📨 Events & Async Contract

**Publishes:** none — `timeline` is a pure read-model materializer.

**Consumes:**

| Topic | Consumer group | Worker / action | On poison/exhaustion |
|---|---|---|---|
| `post.published` | `timeline-post-published` | fan-out (Std/Prem) or VIP-register | DLQ `{topic}.dlq` |
| `post.deleted` | `timeline-post-deleted` | VIP ZREM or Scylla purge | DLQ `{topic}.dlq` |
| `social-graph.followed` | `timeline-sg-followed` | backfill recent posts + update following set | DLQ `{topic}.dlq` |
| `social-graph.unfollowed` | `timeline-sg-unfollowed` | prune posts + update following set | DLQ `{topic}.dlq` |

> **Runtime contract (mandatory):** all workers run under `run_consumer` — manual commit after success,
> bounded retry with backoff + jitter, DLQ on exhaustion/poison. All downstream writes are idempotent
> (ZADD idempotent; Scylla upserts via INSERT).

---

## 🌩️ Failure Modes & Degradation

| Failure | Symptom | Service behavior | Operator action |
|---|---|---|---|
| Redis unavailable / cold | warm reads fail | **Soft** — cold-start serves Scylla (`is_cold=true`), warms async | check Redis; self-heals |
| ScyllaDB unavailable | cold-start + ingest fail | **Hard** for cold path; ingest retries via `run_consumer` | check Scylla; drain DLQ |
| `social-graph` unreachable at boot | following rebuild fails | lazily-connected channel — timeline still boots; `TML-3001` retryable | check social-graph health |
| Tier cache miss | author tier unknown | conservatively routes to `Standard` (no blocking; corrected on next `post.published`) | none — self-correcting |
| Fan-out ingest lag | feed stale | retries within budget | scale the relevant consumer |

**Backpressure & limits.** `TIMELINE_FEED_CAP` (default 500) and `TIMELINE_VIP_REGISTRY_CAP` (200) bound
ZSET size; `TIMELINE_MAX_VIP_MERGE_SOURCES` (50) caps per-request VIP merges; `TIMELINE_MAX_PAGE_SIZE`
clamps pages.

---

## 📦 Integration & Usage

```toml
[dependencies]
timeline = { path = "crates/services/timeline" }
```

Library-only. Implements [`service_runtime::Service`](../../platform/service-runtime/README.md) as
`timeline::service::TimelineService` — `build` maps `TimelineConfig → AppConfig`, constructs the
social-graph gRPC client over a **lazily-connected** channel (timeline boots even if social-graph isn't
reachable yet), assembles cache/persistence adapters + CQRS buses, and spawns the four ingestion
workers; `register` adds the gRPC + reflection services (query-only surface); `health_probes` checks
Scylla/Redis.

### Bootstrap (`crates/apps/timeline-server`)

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

---

## ⚙️ Configuration & Runtime Environment

### `timeline`-specific variables

| Variable | Default | Description |
|---|---|---|
| `TIMELINE_FEED_CAP` | `500` | Max entries per follower's Redis ZSET. |
| `TIMELINE_VIP_REGISTRY_CAP` | `200` | Max entries per VIP author ZSET. |
| `TIMELINE_BACKFILL_LIMIT` | `100` | Max posts backfilled on follow. |
| `TIMELINE_WARM_TTL_SECS` | `86400` | Warm-flag TTL (24 h). |
| `TIMELINE_TIER_CACHE_TTL_SECS` | `3600` | Author tier cache TTL. |
| `TIMELINE_VIP_REGISTRY_TTL_SECS` | `604800` | VIP ZSET TTL (7 d). |
| `TIMELINE_MAX_PAGE_SIZE` | `50` | Max page size. |
| `TIMELINE_MAX_VIP_MERGE_SOURCES` | `50` | Max VIP ZSETs merged per request. |
| `TIMELINE_SOCIAL_GRAPH_PAGE_SIZE` | `500` | Pagination size for social-graph lists. |
| `TIMELINE_SOCIAL_GRAPH_ENDPOINT` | `http://social-graph:50051` | social-graph gRPC endpoint. |
| `TIMELINE_KAFKA_GROUP_*` | `timeline-*` | Consumer group IDs (post-published/deleted, sg-followed/unfollowed). |

> Standard ScyllaDB / Redis / Kafka connection variables from the shared storage crates apply.
> `TIMELINE_GRPC_ADDR` defaults to `0.0.0.0:50060`.

---

## 🚀 Deployment, Migrations & Rollback

- **Migrations:** `0001_create_keyspace.cql` → `0002_create_feed_items_by_profile_table.cql` →
  `0003_create_posts_by_author_table.cql` against `timeline`, applied **before** first start.
- **Stateful gotchas:** `AuthorTier::fan_out_mode()` is a hard invariant, not config — changing tier
  semantics requires a feed rebuild. ZSET member encoding (`{post_id}:{author_id}`) and cursor format are
  read contracts.
- **Rollout/Rollback:** `<TODO>`; the lazily-connected social-graph channel makes boot order tolerant —
  safe to roll.

---

## 📈 Telemetry, Performance & Metrics

- **Runtime:** Tokio multi-thread (VIP merge uses `try_join_all`). Global tracing/OTel subscriber
  installed before `serve`.

| Signal | Why it matters | Suggested alert |
|---|---|---|
| `GetFollowingFeed` p99 (warm) | hot-read SLO | > SLO ⇒ page |
| `is_cold` rate | Redis warm-coverage | sustained high ⇒ check warming / Redis evictions |
| fan-out consumer lag | feed freshness | > threshold ⇒ scale consumers |
| `TML-3001` rate | social-graph dependency health | spike ⇒ check social-graph |
| DLQ produce rate (`{topic}.dlq`) | poison / retry-exhausted | any sustained rate ⇒ page |

---

## 🛠️ Local Development

```bash
docker compose up -d scylladb redis kafka     # repo-root compose
for f in crates/services/timeline/migrations/*.cql; do cqlsh -f "$f"; done
cargo build -p timeline && cargo clippy -p timeline --all-targets
cargo test  -p timeline
```

---

## 🚨 Troubleshooting & Runbook

> Format: **symptom → root cause → mitigation.**

**1. A VIP author's posts don't appear in followers' feeds.**
Root cause: this is by design — VIP posts are *not* fanned out; they live in `timeline:vip:{author}` and
are merged at query time. If they're missing from the merged result, check `TIMELINE_MAX_VIP_MERGE_SOURCES`
(the follower may follow more VIPs than the merge cap) or the VIP ZSET TTL. Mitigation: confirm
`ZCARD timeline:vip:{author}` > 0; raise the merge cap if a user follows many VIPs.

**2. `GetFollowingFeed` keeps returning `is_cold=true`.**
Root cause: the warm flag (`timeline:warm:{profile}`) keeps expiring or the async warm task is failing —
often Redis eviction pressure or a `ScriptReturnInvalid` (TML-5001) in the warm path. Mitigation: check
Redis `maxmemory`/eviction and the warm-task logs; the cold path is still correct (served from Scylla),
just slower.

**3. A new follow's posts don't show up (no backfill).**
Root cause: the `social-graph.followed` event was consumed but backfill failed (`TML-5002`), or the
followee is VIP (no backfill — merged live instead). Mitigation: check `timeline-sg-followed` lag/DLQ;
verify the followee's tier — VIP follows are correct to skip backfill.
