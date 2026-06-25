# `geo-discovery` — Real-time H3 spatial index for the global map feed, served in two Redis round-trips

> **Service Card**
>
> | | |
> |---|---|
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |
> | **On-call / escalation** | `<TODO: oncall-rotation>` → `<TODO: escalation-policy>` |
> | **Tier** | **TIER-1** — query-only read surface; degradable to ScyllaDB |
> | **Deployable** | `crates/apps/geo-discovery-server` (library crate: `crates/services/geo-discovery`) |
> | **Datastores** | Redis (ZSET index + msgpack card cache) · ScyllaDB keyspace `geo_discovery` |
> | **Async** | publishes nothing · consumes `post.published` / `engagement.score_updated` / `profile.tier_changed` |
> | **Upstream callers** | `<TODO: BFF / map clients>` |
> | **Downstream deps** | Redis, ScyllaDB, Kafka |
> | **SLO** | tile query p99 **< 50 ms** at continental scale |

---

## 🎯 Overview & Service Role

`geo-discovery` is the geospatial ingestion + query engine behind the global interactive map. Every
published post is encoded into Uber H3 hexagonal cells at three resolutions, scored by real-time
virality, and served as a fully-hydrated map card in **two Redis round-trips** — with no fan-out to
`post` or `profile` at query time.

The hard problem it solves is that **a global map feed with 100 M active posts cannot afford N+1 gRPC
lookups on every viewport pan**. It resolves this with an independent read-model projection: card
fields (`author_handle`, `thumbnail_url`, `author_avatar_url`, `author_tier`) are denormalized at
ingest, so rendering data is entirely local. Dynamic relational state (friend/following) is resolved
client-side, preserving a *shared* card cache and avoiding O(users × posts) cache variants.

**Core objectives:** sub-50 ms P99 tile query (Redis pipeline + MGET); ~9 GB Redis at 100 M posts
(Top-K cap + cold eviction + power-law filtering); zero query-time fan-out; 48 h default TTL (30 d for
premium), enforced via Scylla `USING TTL` and Redis `EX`.

---

## 📐 Architecture & Concepts

The gRPC surface is **query-only** — all writes arrive via Kafka workers.

```
WRITE: post.published          ─► PostIndexerWorker  (H3 encode R5/7/9 → Scylla INSERT ×4 → Redis ZADD+cap ×3 → card SET if score≥θ)
       engagement.score_updated ─► ScoreUpdaterWorker (Scylla UPDATE score → ZADD XX ×3, skip-if-absent)
       profile.tier_changed     ─► TierSyncWorker     (Scylla UPDATE author_tier → Redis DEL card)
       (60s tick)               ─► TilePrunerWorker    (PRUNE_COLD_TILES Lua → DEL cold tile ZSETs)

READ:  QueryTile ─► zoom→resolution ─► viewport→grid_disk (≤50 tiles)
                 ─► Phase 1: ZRANGEBYSCORE ×N (1 RTT via fred mux) → post_ids
                 ─► Phase 2: MGET cards ×M (1 RTT)
                 ─► Phase 3 (miss): Scylla get_card (Fast profile)
```

**Redis taxonomy:** `sg:geo:tile:{h3}:{res}` (ZSET, score=virality, pruned), `sg:geo:card:{post_id}`
(STRING, msgpack `MapPostCard`, `EX ttl`), `sg:geo:hot_tiles` (ZSET, last-access epoch per tile).
**ScyllaDB:** `posts_by_tile` (TWCS, PK `(h3_index, resolution)` — composite to avoid hot urban shards),
`map_post_cards` (LCS, PK `post_id` — pure point reads, single mutable score column).

Three atomic Lua scripts drive the hot path: `ZADD_TOPK` (cap per tile, evict lowest on overflow),
`ZADD_XX` (update only if member present — evicted posts never re-inserted), `PRUNE_COLD_TILES` (evict
tiles idle past the cold threshold).

> ⚠️ **Cluster note:** `PRUNE_COLD_TILES` builds tile keys inside Lua and is **not Redis Cluster-safe**
> (cross-slot `DEL`). This service assumes standalone / single-shard Redis.

> **Invariants:** zoom→resolution mapping with virality floors (R5 floor 500 / R7 50 or 5 / R9 0) and
> Top-K caps (200/500/1000) bound per-tile RAM regardless of urban density.

---

## 📊 Service Level Objectives (SLO)

| SLI | Objective | Window | Measured by |
|---|---|---|---|
| `QueryTile` p99 | **< 50 ms** | 1h | `geo_discovery_tile_query_duration_ms` |
| Cache miss ratio | < 0.30 | 5m | `geo_discovery_cache_miss_ratio` |
| `post.published` ingest lag | < 30 s | live | `geo_discovery_post_indexer_lag_seconds` |
| `engagement.score_updated` lag | < 10 s | live | `geo_discovery_score_updater_lag_seconds` |
| Redis spatial RAM (hot tiles) | < 50 000 tiles | live | `geo_discovery_hot_tile_count` |

**Error budget:** `<TODO>`. **On burn:** `<TODO>`. Map data is acceptably stale within the 48 h
retention window, so ingest-lag SLOs are softer than the query-latency SLO.

---

## 🔗 Dependencies & Blast Radius

**Downstream:**

| Dependency | Purpose | If down → | Degradation |
|---|---|---|---|
| Redis | ZSET index + card cache | query latency rises | **Soft** — reads fall back to Scylla (not an outage) |
| ScyllaDB (`geo_discovery`) | durable source of truth | ingest retries; cold reads fail | **Hard** for cold path; Redis-served queries unaffected |
| Kafka | ingest (post/score/tier) | map data goes stale | **Soft** — queries still serve cached data |

**Upstream (blast radius):**

| Caller | Uses | Impact if `geo-discovery` is down |
|---|---|---|
| `<TODO: BFF / map clients>` | `QueryTile`, `GetCard` | the map feed stops loading |

> **Critical path?** Yes for the map surface specifically; it is a derived read-model, so a full outage
> degrades the map but nothing else.

---

## 🔌 Public Interfaces & API Contract

### gRPC — `geo_discovery.v1.GeoDiscoveryService`

```protobuf
service GeoDiscoveryService {
  rpc QueryTile (QueryTileRequest) returns (QueryTileResponse);
  rpc GetCard   (GetCardRequest)   returns (GetCardResponse);
}
message QueryTileRequest { Viewport viewport = 1; int32 zoom_level = 2; }  // zoom ∈ [0,15]
message MapPostCard { string post_id=1; string author_id=2; string author_handle=3;
  string author_avatar_url=4; string thumbnail_url=5; int64 h3_index_r7=6;
  float virality_score=7; int64 published_at_ms=8; AuthorTier author_tier=9; }
```

> **Wire contract:** `AuthorTier` is 0-based **with** an `UNSPECIFIED=0` safe default (= Standard);
> `STANDARD=1, PREMIUM=2, VIP=3`. Badge rendering: `author_tier` → static badge; `is_friend`/`is_following`
> are deliberately **absent** (resolved client-side from the session social graph).

### Rust ports (hexagonal contract)

```rust
pub trait SpatialIndex: Send + Sync { /* upsert (ZADD+cap), update_score (ZADD XX), query (ZRANGEBYSCORE), touch_hot_tiles */ }
pub trait CardStore:    Send + Sync { /* set, mget (same-length Vec, None=miss), del */ }
pub trait TileRepository: Send + Sync { /* insert_tile_entry, upsert_card, update_card_score/tier, get_card, list_tile_post_ids */ }
```

### Error contract (`GEO-xxxx`)

| Code | HTTP | Meaning |
|---|---|---|
| GEO-1001/1002 | 422 | coords outside WGS-84 / invalid H3 index |
| GEO-2001/2002 | 422 | viewport SW≥NE / zoom outside [0,15] |
| GEO-4001 | 500 | Lua returned unexpected value |
| GEO-5001/5002 | 500 | msgpack ser / deser failure |
| GEO-9001..9003 | 422 | malformed UUIDs / domain violation |

---

## 📨 Events & Async Contract

**Publishes:** none — `geo-discovery` is a pure read-model materializer.

**Consumes:**

| Topic | Consumer group | Purpose | On poison/exhaustion |
|---|---|---|---|
| `post.published` | `geo-discovery-post-indexer` | H3 index + card projection | DLQ `{topic}.dlq` |
| `engagement.score_updated` | `geo-discovery-score-updater` | virality score sync (ZADD XX) | DLQ `{topic}.dlq` |
| `profile.tier_changed` | `geo-discovery-tier-sync` | author tier sync + card invalidation (one event per `post_id`, stateless) | DLQ `{topic}.dlq` |

> **Runtime contract (mandatory):** all three workers run under `run_consumer` — retry in place with
> backoff + jitter (≤5 attempts), dead-letter on exhaustion and commit past it so a partition never
> stalls. At-least-once; all writes idempotent. Scylla is the durable source of truth; Redis ZSETs
> repopulate on replay from `earliest`.

---

## 🌩️ Failure Modes & Degradation

| Failure | Symptom | Service behavior | Operator action |
|---|---|---|---|
| Redis unavailable | query latency up | **Soft** — Scylla write succeeds; query degrades to full Scylla reads | auto-recovers on reconnect |
| ScyllaDB unavailable | ingest retries | `run_consumer` retries→DLQ; cold reads fail | drain DLQ once Scylla recovers |
| Consumer lag | map data stale | query path unaffected (Redis/Scylla reads) | scale consumer replicas |
| Redis memory pressure | `hot_tile_count` climbs | TilePruner evicts every 60 s; Top-K cap bounds per-tile | lower `GEO_TILE_COLD_THRESHOLD_SECS` |
| Score event storm | — | `ZADD_XX` skips absent members; no ZSET inflation | self-limiting |

**Backpressure & limits.** Top-K cap on every `ZADD`; cold-tile eviction every 60 s; viewport capped at
≤50 H3 tiles per query. Writes use Strict (`LocalQuorum`), reads use Fast (`LocalOne` + speculative).

---

## 📦 Integration & Usage

```toml
[dependencies]
geo-discovery = { path = "crates/services/geo-discovery" }
```

Library-only. Implements [`service_runtime::Service`](../../platform/service-runtime/README.md) as
`geo_discovery::service::GeoDiscoveryService` — `build` constructs Scylla/Redis clients, instantiates
`RedisGeoSpatialIndex`/`RedisCardStore`/`ScyllaTileRepository`, registers `QueryTileHandler` (query-only
surface; writes arrive via Kafka), and spawns the three workers + `TilePrunerWorker`; `register` adds
the gRPC + reflection services; `health_probes` checks Scylla/Redis.

### Bootstrap (`crates/apps/geo-discovery-server`)

```rust
use std::net::SocketAddr;
use geo_discovery::service::GeoDiscoveryService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("GEO_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50054".to_owned())
        .parse()?;
    service_runtime::serve::<GeoDiscoveryService>(addr).await
}
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_URI` | **Yes** | — | ScyllaDB contact points. |
| `REDIS_URL` | **Yes** | — | Redis connection URL (standalone / single-shard). |
| `KAFKA_BROKERS` | **Yes** | — | Kafka brokers. |
| `GEO_GRPC_ADDR` | No | `0.0.0.0:50054` | gRPC bind address. |
| `GEO_CARD_CACHE_THRESHOLD` | No | `10.0` | Min virality to cache a card (power-law filter). |
| `GEO_DEFAULT_RETENTION_SECS` | No | `172800` | Default post TTL (48 h). **Must match Scylla `default_time_to_live`.** |
| `GEO_TILE_PRUNER_INTERVAL_SECS` | No | `60` | Cold-tile eviction tick. |
| `GEO_TILE_COLD_THRESHOLD_SECS` | No | `1800` | Inactivity window before a tile ZSET is evicted. |
| `GEO_POST_INDEXER_GROUP_ID` / `GEO_SCORE_UPDATER_GROUP_ID` / `GEO_TIER_SYNC_GROUP_ID` | No | service-specific | Kafka consumer groups. |

> No compile-time feature flags. `build.rs` compiles `proto/geo_discovery/v1/*.proto`. ScyllaDB profiles:
> Strict (`LocalQuorum`) for mutations, Fast (`LocalOne` + speculative) for reads.

---

## 🚀 Deployment, Migrations & Rollback

- **Migrations:** `0001_create_keyspace.cql` → `0002_create_posts_by_tile_table.cql` →
  `0003_create_map_post_cards_table.cql` against `geo_discovery`, applied **before** first start.
- **Stateful gotchas:** `GEO_DEFAULT_RETENTION_SECS` must equal the Scylla table TTL; the composite
  `(h3_index, resolution)` partition key and zoom→resolution mapping are read contracts.
- **Cold-start:** workers replay from `earliest`; Redis ZSETs repopulate automatically. Safe to roll.

---

## 📈 Telemetry, Performance & Metrics

- **Runtime:** Tokio multi-thread (required — `tokio::join!` concurrent Scylla+Redis writes). `h3o` is
  pure Rust. Memory floor ~512 MB; `ulimit -n ≥ 4096`.

| Signal | Why it matters | Suggested alert |
|---|---|---|
| `geo_discovery_tile_query_duration_ms` | query SLO | p99 > 50 ms ⇒ page |
| `geo_discovery_cache_miss_ratio` | Redis offload health | > 0.30 for 5m ⇒ investigate |
| `geo_discovery_hot_tile_count` | spatial RAM | > 50 000 ⇒ tune pruner |
| `geo_discovery_post_indexer_lag_seconds` | ingest freshness | > 30 s ⇒ scale consumers |
| `geo_discovery_card_serialization_errors` | schema/memory | > 0 in 1m ⇒ investigate |

**Redis memory budget (reference):** ~0.7 GB at 10 M posts, ~7 GB at 100 M. Levers: per-resolution
`top_k_cap`, `GEO_CARD_CACHE_THRESHOLD`, `GEO_TILE_COLD_THRESHOLD_SECS`.

---

## 🛠️ Local Development

```bash
docker compose up -d scylla redis kafka       # repo-root compose
for f in crates/services/geo-discovery/migrations/*.cql; do cqlsh 127.0.0.1 9042 -f "$f"; done
cargo build -p geo-discovery && cargo clippy -p geo-discovery -- -D warnings
cargo test  -p geo-discovery
SCYLLA_URI=127.0.0.1:9042 REDIS_URL=redis://127.0.0.1:6379 KAFKA_BROKERS=127.0.0.1:9092 cargo run -p geo-discovery
```

---

## 🚨 Troubleshooting & Runbook

> Format: **symptom → root cause → mitigation.**

**1. `QueryTile` returns `tile_count > 0` but empty `cards`.**
Root cause (most common): ZSETs empty but Scylla has rows ⇒ the `geo-discovery-post-indexer` is lagging;
or ZSETs populated but cards empty ⇒ `GEO_CARD_CACHE_THRESHOLD` too high. Mitigation: check
`kafka-consumer-groups --describe --group geo-discovery-post-indexer` and scale; or lower the threshold
to `1.0` temporarily to confirm cards appear. Verify the client isn't sending inverted SW/NE coords.

**2. Redis memory growing without bound.**
Root cause: TilePruner crashed (check for `tile pruner worker started`; `geo_discovery_tile_pruner_evictions`
stuck at 0) or `GEO_TILE_COLD_THRESHOLD_SECS` too high. Mitigation: restart/redeploy; lower the cold
threshold to `900`. Emergency: `redis-cli FLUSHDB` (announce maintenance) — next queries cold-start from
Scylla.

**3. Score updates not reflected on the map.**
Root cause: the post was Top-K evicted (`ZADD_XX` skips absent members — expected; refreshes on TTL), or
`geo-discovery-score-updater` has consumer lag. Mitigation: compare Scylla `map_post_cards.virality_score`
to the Redis `ZSCORE`; if Scylla is stale too, scale the score-updater consumer.
