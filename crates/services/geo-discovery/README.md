# geo-discovery — Real-time H3 spatial indexing engine for the global map feed

## 🎯 Overview & Service Role

`geo-discovery` is the **geospatial ingestion and query engine** that powers the app's global interactive map. Every published post is encoded into Uber H3 hexagonal cells at three resolutions, scored by real-time virality, and served as a fully hydrated map card in **two Redis round-trips** — with no fan-out to `services/post` or `services/profile`.

**Critical problem solved:** A global map feed with 100 M active posts cannot afford N+1 gRPC lookups on every viewport pan. This service maintains its own independent read-model projection, keeping all BFF rendering data local.

**Business impact:**
- **Sub-50 ms P99 tile query** at continental scale via Redis pipeline + MGET.
- **~9 GB Redis steady-state** at 100 M active posts (Top-K capping + cold eviction + power-law filtering).
- **Zero post-query fan-out**: `author_handle`, `thumbnail_url`, `author_avatar_url`, and `author_tier` are projected here, never fetched at query time.
- **48 h default TTL**, extensible to 30 days for premium creators, enforced natively via ScyllaDB `USING TTL` and Redis `EX`.
- **Author tier badge rendering with zero cross-service calls**: static `author_tier` (Standard / Premium / VIP) is denormalized into the shared card projection; dynamic relational state (friend / following) is resolved client-side from the session social graph, preserving the shared-card cache model and eliminating O(users × posts) cache variants.

---

## 📐 Architecture & Concepts

### Full System Data Flow

```
═══════════════════════════════ WRITE PATH ══════════════════════════════════

  services/post          Kafka topic             PostIndexerWorker
  ┌────────────┐         post.published           ┌──────────────────────┐
  │ PublishPost│──────►  { post_id, lat, lng, ───►│ 1. H3 encode (R5/7/9)│
  └────────────┘         author_handle,           │ 2. ScyllaDB INSERT ×4│
                         thumbnail_url,           │    (3 tile rows +    │
                         virality_score,          │     1 card row)      │
                         retention_secs }         │ 3. Redis ZADD+cap ×3 │
                                                  │ 4. Redis SET card    │
                                                  │    (if score ≥ θ)    │
                                                  └──────────────────────┘

  services/engagement    Kafka topic             ScoreUpdaterWorker
  ┌────────────┐         engagement.score_updated  ┌──────────────────────┐
  │ score event│──────►  { post_id, new_score } ──►│ 1. ScyllaDB get_card │
  └────────────┘                                   │    (resolve h3_index)│
                                                  │ 2. ScyllaDB UPDATE   │
                                                  │    virality_score    │
                                                  │ 3. ZADD XX ×3 (Redis)│
                                                  └──────────────────────┘

  services/profile       Kafka topic             TierSyncWorker
  ┌────────────┐         profile.tier_changed      ┌──────────────────────┐
  │ tier change│──────►  { author_id, post_id, ───►│ 1. ScyllaDB UPDATE   │
  └────────────┘         new_tier }                │    author_tier       │
                         (1 event per post_id)     │ 2. Redis DEL card    │
                                                  │    (cache invalidate) │
                                                  └──────────────────────┘

  TilePrunerWorker (60 s tick)
  ┌──────────────────────────────────────────────────────────┐
  │ PRUNE_COLD_TILES Lua → DEL cold tile ZSETs               │
  │ Evicts tiles with last-access epoch ≤ (now − 30 min)    │
  └──────────────────────────────────────────────────────────┘

═══════════════════════════════ READ PATH ═══════════════════════════════════

  BFF / client                                    GeoDiscoveryHandler
  ┌──────────────┐                                ┌───────────────────────┐
  │ QueryTile    │──────► gRPC QueryTileRequest ──►│ zoom → resolution     │
  │ viewport +   │        { viewport, zoom }       │ viewport → grid_disk  │
  │ zoom level   │                                 │ (≤50 H3 tiles)        │
  └──────────────┘                                 │                       │
          ▲                                        │ Phase 1:              │
          │                                        │   ZRANGEBYSCORE × N   │
          │                                        │   tiles (concurrent,  │
          │                                        │   1 RTT via fred mux) │
          │                                        │   → post_id list      │
          │                                        │                       │
          │                                        │ Phase 2:              │
          └──────── QueryTileResponse ─────────────│   MGET cards × M      │
               { cards: [MapPostCard] }            │   (1 RTT)             │
                                                  │                       │
                                                  │ Phase 3 (cache miss): │
                                                  │   ScyllaDB get_card   │
                                                  │   (Fast profile)      │
                                                  └───────────────────────┘
```

### Redis Key Taxonomy

| Key Pattern                    | Type   | TTL              | Purpose                                           |
|--------------------------------|--------|------------------|---------------------------------------------------|
| `sg:geo:tile:{h3_u64}:{res}`   | ZSET   | None (pruned)    | Spatial index — score = virality, member = post_id |
| `sg:geo:card:{post_id}`        | STRING | `EX {ttl_secs}`  | Msgpack-encoded `MapPostCard`                     |
| `sg:geo:hot_tiles`             | ZSET   | None             | Last-access Unix epoch per tile suffix            |

### ScyllaDB Schema Summary

| Table             | Compaction | Partition Key             | Clustering Key                         | Role                                         |
|-------------------|------------|---------------------------|----------------------------------------|----------------------------------------------|
| `posts_by_tile`   | TWCS 1 d   | `(h3_index, resolution)`  | `(published_at DESC, post_id ASC)`     | Cold-start ZSET reconstruction               |
| `map_post_cards`  | LCS        | `post_id`                 | —                                      | Card point-reads, score updates, tier updates |

**Mutable columns in `map_post_cards`:**

| Column          | Updated by           | Trigger                              |
|-----------------|----------------------|--------------------------------------|
| `virality_score`| `ScoreUpdaterWorker` | `engagement.score_updated` Kafka topic |
| `author_tier`   | `TierSyncWorker`     | `profile.tier_changed` Kafka topic   |

**Why `(h3_index, resolution)` as composite partition key?**
A viral urban tile at resolution 9 (street level, ~0.1 km²) can accumulate millions of posts. Collapsing all resolutions into one partition creates a prohibitively hot shard. The composite key distributes each `(tile × resolution)` pair across the ring independently.

**Why LCS for `map_post_cards`?**
Access is exclusively by `post_id` — pure point-reads with a single mutable column (`virality_score`). TWCS would increase read amplification for point lookups without providing SSTable-drop benefits. LCS minimises read amplification for this workload.

### Zoom → Resolution Mapping

| Client Zoom | H3 Resolution | Tile Area   | Virality Floor | Top-K Cap | Rationale                              |
|-------------|---------------|-------------|----------------|-----------|----------------------------------------|
| 1–4         | R5            | ~87 km²     | **500**        | 200       | Continental: only breakthrough content |
| 5–8         | R7            | ~5 km²      | **50**         | 500       | Metro: curated high-traffic feed       |
| 9–12        | R7            | ~5 km²      | **5**          | 500       | Neighbourhood: broad local content     |
| 13–15       | R9            | ~0.1 km²    | **0**          | 1 000     | Street level: every post visible       |

### Lua Script Inventory

Three atomic Redis scripts drive the critical-path operations:

**`ZADD_TOPK_SCRIPT`** — used on every post index write (3× per publish event):
```lua
-- KEYS[1] = tile ZSET key
-- ARGV[1] = score, ARGV[2] = post_id, ARGV[3] = top_k cap
redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
local size = redis.call('ZCARD', KEYS[1])
if size > tonumber(ARGV[3]) then
    redis.call('ZREMRANGEBYRANK', KEYS[1], 0, size - tonumber(ARGV[3]) - 1)
end
return redis.call('ZCARD', KEYS[1])
```
*Effect: single atomic round-trip. Evicts the lowest-score member(s) when the per-tile cap is exceeded, bounding RAM regardless of urban density.*

**`ZADD_XX_SCRIPT`** — used on every score update (3× per score event):
```lua
-- KEYS[1] = tile ZSET key
-- ARGV[1] = new_score, ARGV[2] = post_id
local exists = redis.call('ZSCORE', KEYS[1], ARGV[2])
if exists ~= false then
    redis.call('ZADD', KEYS[1], ARGV[1], ARGV[2])
    return 1
end
return 0
```
*Effect: updates score only if the post is present. Evicted posts are not re-inserted. Preserves the Top-K invariant.*

**`PRUNE_COLD_TILES_SCRIPT`** — used by `TilePrunerWorker` every 60 s:
```lua
-- KEYS[1] = sg:geo:hot_tiles
-- ARGV[1] = cutoff_epoch, ARGV[2] = batch_size, ARGV[3] = key prefix
local cold = redis.call('ZRANGEBYSCORE', KEYS[1], '-inf', ARGV[1], 'LIMIT', '0', ARGV[2])
if #cold == 0 then return 0 end
for _, suffix in ipairs(cold) do
    redis.call('DEL', ARGV[3] .. suffix)
end
redis.call('ZREMRANGEBYSCORE', KEYS[1], '-inf', ARGV[1])
return #cold
```
*Effect: single pass evicts up to `batch_size` cold tile ZSETs and removes them from the tracker. Bounded latency per tick.*

> ⚠️ **Cluster note:** `PRUNE_COLD_TILES_SCRIPT` constructs tile keys inside Lua and is not Redis Cluster–safe (cross-slot `DEL`). This service assumes standalone Redis or a single-shard deployment.

### Resilience Guarantees & High-Load Behavior

| Failure Scenario | Behaviour | Recovery |
|---|---|---|
| **Redis unavailable** | ScyllaDB write succeeds; Redis write logs a warning and is skipped. Query path degrades to full ScyllaDB reads (higher latency, not an outage). | Automatic on reconnect. |
| **ScyllaDB unavailable** | Kafka consumer fails with error; restarts with 5 s exponential backoff. Message is redelivered (at-least-once). | Automatic; all writes are idempotent. |
| **Kafka consumer lag** | gRPC query path is unaffected (reads only Redis/ScyllaDB). Map data is temporarily stale — acceptable within the 48 h retention window. | Scale consumer replicas. |
| **Score event storm** | `ZADD_XX_SCRIPT` skips absent members; no ZSET inflation. ScyllaDB UPDATE is last-write-wins. | Inherently self-limiting. |
| **Redis memory pressure** | `TilePrunerWorker` evicts cold tiles every 60 s. Top-K cap on every ZADD prevents per-tile bloat. | Lower `GEO_TILE_COLD_THRESHOLD_SECS`; check alert `hot_tile_count > 50 000`. |
| **Service restart** | `PostIndexerWorker` replays from `earliest` offset; Redis ZSETs are repopulated automatically. ScyllaDB is always the durable source of truth. | Automatic; bounded by Kafka retention window. |

---

## 🔌 Public Interfaces & API Contract

### Protobuf Contract

```protobuf
// enums.proto
enum AuthorTier {
    AUTHOR_TIER_UNSPECIFIED = 0;  // Safe zero-value default (= Standard)
    AUTHOR_TIER_STANDARD    = 1;
    AUTHOR_TIER_PREMIUM     = 2;
    AUTHOR_TIER_VIP         = 3;
}

// service.proto
service GeoDiscoveryService {
    rpc QueryTile (QueryTileRequest) returns (QueryTileResponse);
    rpc GetCard   (GetCardRequest)   returns (GetCardResponse);
}

// messages.proto
message Viewport {
    double sw_lat = 1;  // [-90, 90]
    double sw_lng = 2;  // [-180, 180]
    double ne_lat = 3;  // [-90, 90]
    double ne_lng = 4;  // [-180, 180]
}

message QueryTileRequest {
    Viewport viewport   = 1;
    int32    zoom_level = 2;  // [0, 15]
}

message QueryTileResponse {
    repeated MapPostCard cards      = 1;
    int32                tile_count = 2;  // H3 tiles queried (for client telemetry)
}

// Badge rendering contract:
//   author_tier  → static badge (VIP gold border / Premium purple border)
//   is_friend    → NOT present: resolved client-side from session social graph
//   is_following → NOT present: resolved client-side from session social graph
message MapPostCard {
    string     post_id           = 1;
    string     author_id         = 2;
    string     author_handle     = 3;
    string     author_avatar_url = 4;
    string     thumbnail_url     = 5;
    int64      h3_index_r7       = 6;   // raw H3 cell index for deep-link map centering
    float      virality_score    = 7;
    int64      published_at_ms   = 8;   // Unix epoch milliseconds
    AuthorTier author_tier       = 9;   // static; kept current via TierSyncWorker
}

message GetCardRequest  { string post_id = 1; }
message GetCardResponse { MapPostCard card = 1; bool found = 2; }
```

### Domain Entity

```rust
// src/domain/entity/map_post_card.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPostCard {
    pub post_id:           Uuid,
    pub author_id:         Uuid,
    pub author_handle:     String,
    pub author_avatar_url: String,
    pub thumbnail_url:     String,
    pub h3_index_r7:       i64,    // raw H3 u64 cast to i64 (bit 63 always 0)
    pub virality_score:    f32,
    pub published_at_ms:   i64,    // Unix epoch ms
    #[serde(default)]              // missing in legacy msgpack → decoded as 0 (Standard)
    pub author_tier:       u8,     // 0=Standard, 1=Premium, 2=VIP
}
```

### Kafka Event Schemas Consumed

```rust
// Topic: post.published  →  triggers PostIndexerWorker
#[derive(Deserialize)]
pub struct PostPublishedEvent {
    pub post_id:           String,  // UUID hyphenated
    pub author_id:         String,
    pub author_handle:     String,
    pub author_avatar_url: String,
    pub thumbnail_url:     String,
    pub lat:               f64,
    pub lng:               f64,
    pub virality_score:    f64,    // typically 0.0 for new posts
    pub published_at_ms:   i64,
    pub retention_secs:    Option<u64>,  // None → 172 800 s (48 h)
    #[serde(default)]
    pub author_tier:       u8,     // 0=Standard, 1=Premium, 2=VIP. Absent → 0.
}

// Topic: engagement.score_updated  →  triggers ScoreUpdaterWorker
#[derive(Deserialize)]
pub struct ScoreUpdatedEvent {
    pub post_id:   String,
    pub new_score: f64,
}

// Topic: profile.tier_changed  →  triggers TierSyncWorker
// services/profile emits ONE event per affected post_id (not one per author)
// so this consumer requires no author→posts index and stays stateless.
#[derive(Deserialize)]
pub struct TierChangedEvent {
    pub author_id: String,  // informational; used for tracing only
    pub post_id:   String,  // post whose card projection must be updated
    pub new_tier:  u8,      // 0=Standard, 1=Premium, 2=VIP
}
```

### Application Port Traits

```rust
// Port 1: Redis ZSET spatial index
pub trait SpatialIndex: Send + Sync {
    // Atomic ZADD + Top-K cap via Lua. Called 3× per publish event (R5/R7/R9).
    async fn upsert(&self, tile: H3Index, res: H3Resolution,
                    post_id: &PostId, score: ViralityScore) -> Result<(), GeoDiscoveryError>;

    // ZADD XX via Lua — updates score only if member exists. Returns true if updated.
    async fn update_score(&self, tile: H3Index, res: H3Resolution,
                          post_id: &PostId, score: ViralityScore) -> Result<bool, GeoDiscoveryError>;

    // ZRANGEBYSCORE ≥ min_score, returns post UUIDs.
    async fn query(&self, tile: H3Index, res: H3Resolution,
                   min_score: f64) -> Result<Vec<Uuid>, GeoDiscoveryError>;

    // ZADD hot_tiles with current epoch (fire-and-forget).
    async fn touch_hot_tiles(&self, tiles: &[(H3Index, H3Resolution)]) -> Result<(), GeoDiscoveryError>;
}

// Port 2: Redis msgpack card cache
pub trait CardStore: Send + Sync {
    async fn set(&self, card: &MapPostCard, ttl: RetentionTtl) -> Result<(), GeoDiscoveryError>;
    // MGET — returns same-length Vec, None = cache miss.
    async fn mget(&self, post_ids: &[Uuid]) -> Result<Vec<Option<MapPostCard>>, GeoDiscoveryError>;
    async fn del(&self, post_id: &PostId) -> Result<(), GeoDiscoveryError>;
}

// Port 3: ScyllaDB durable persistence
pub trait TileRepository: Send + Sync {
    async fn insert_tile_entry(&self, h3_index: H3Index, resolution: H3Resolution,
                               post_id: &PostId, published_at_ms: i64,
                               ttl: RetentionTtl) -> Result<(), GeoDiscoveryError>;
    async fn upsert_card(&self, card: &MapPostCard, ttl: RetentionTtl) -> Result<(), GeoDiscoveryError>;
    async fn update_card_score(&self, post_id: &PostId, score: f32) -> Result<(), GeoDiscoveryError>;
    async fn update_card_tier(&self, post_id: &PostId, tier: i8) -> Result<(), GeoDiscoveryError>;
    async fn get_card(&self, post_id: &PostId) -> Result<Option<MapPostCard>, GeoDiscoveryError>;
    async fn list_tile_post_ids(&self, h3_index: H3Index, resolution: H3Resolution,
                                limit: i32) -> Result<Vec<Uuid>, GeoDiscoveryError>;
}
```

### Error Code Namespace

| Code     | HTTP   | Severity | Meaning                               |
|----------|--------|----------|---------------------------------------|
| GEO-1001 | 422    | Medium   | Coordinate outside WGS-84 bounds      |
| GEO-1002 | 422    | Low      | Invalid H3 cell index value           |
| GEO-2001 | 422    | Medium   | Viewport SW corner ≥ NE corner        |
| GEO-2002 | 422    | Low      | Zoom level outside [0, 15]            |
| GEO-4001 | 500    | High     | Lua script returned unexpected value  |
| GEO-5001 | 500    | High     | Msgpack serialization failure         |
| GEO-5002 | 500    | High     | Msgpack deserialization failure       |
| GEO-9001 | 422    | Low      | Malformed post UUID                   |
| GEO-9002 | 422    | Low      | Malformed author UUID                 |
| GEO-9003 | 422    | Medium   | Domain invariant violated             |

---

## 📦 Integration & Usage

```toml
# In a binary crate (e.g. an API server entry point)
[dependencies]
geo-discovery = { path = "crates/services/geo-discovery" }
telemetry     = { path = "crates/shared/telemetry" }
tokio         = { workspace = true }
```

### Standard Bootstrap Pattern

```rust
// src/main.rs
use std::net::SocketAddr;
use geo_discovery::infrastructure::grpc::server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialise OTel tracing + metrics from env.
    telemetry::init_from_env("geo-discovery")?;

    let addr: SocketAddr = std::env::var("GEO_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50054".to_owned())
        .parse()?;

    // serve() is blocking: it:
    //   1. Builds ScyllaDB CachingSession (ScyllaSessionBuilder).
    //   2. Builds RedisClient (RedisClientBuilder).
    //   3. Instantiates RedisGeoSpatialIndex, RedisCardStore, ScyllaTileRepository.
    //   4. Registers QueryTileHandler in the CQRS QueryBus.
    //   5. Spawns PostIndexerWorker, ScoreUpdaterWorker, TilePrunerWorker.
    //   6. Starts Tonic gRPC server with health + reflection.
    server::serve(addr).await
}
```

**Minimal environment for local startup:**
```bash
export SCYLLA_URI="127.0.0.1:9042"
export REDIS_URL="redis://127.0.0.1:6379"
export KAFKA_BROKERS="127.0.0.1:9092"
cargo run -p geo-discovery
```

---

## ⚙️ Configuration & Runtime Environment

### Required Environment Variables

| Variable         | Required | Description                          |
|------------------|----------|--------------------------------------|
| `SCYLLA_URI`     | **Yes**  | Comma-separated ScyllaDB contact points (e.g. `10.0.0.1:9042,10.0.0.2:9042`) |
| `REDIS_URL`      | **Yes**  | Redis connection URL (e.g. `redis://10.0.0.3:6379`) |
| `KAFKA_BROKERS`  | **Yes**  | Kafka bootstrap broker list (e.g. `10.0.0.4:9092`) |

### Optional Tuning Variables

| Variable                        | Default                        | Production-Safe Default | Description                                                         |
|---------------------------------|--------------------------------|-------------------------|---------------------------------------------------------------------|
| `GEO_GRPC_ADDR`                 | `0.0.0.0:50054`                | `0.0.0.0:50054`         | gRPC listen address                                                 |
| `GEO_CARD_CACHE_THRESHOLD`      | `10.0`                         | `10.0`                  | Minimum virality score for card Redis caching (power-law filter)    |
| `GEO_DEFAULT_RETENTION_SECS`    | `172800`                       | `172800`                | Default post TTL in seconds (48 h). Must match ScyllaDB `default_time_to_live`. |
| `GEO_TILE_PRUNER_INTERVAL_SECS` | `60`                           | `60`                    | Cold-tile eviction tick interval in seconds                         |
| `GEO_TILE_COLD_THRESHOLD_SECS`  | `1800`                         | `1800`                  | Inactivity window before a tile ZSET is evicted (30 min)           |
| `GEO_POST_INDEXER_GROUP_ID`     | `geo-discovery-post-indexer`   | service-specific        | Kafka consumer group for `post.published`                           |
| `GEO_SCORE_UPDATER_GROUP_ID`    | `geo-discovery-score-updater`  | service-specific        | Kafka consumer group for `engagement.score_updated`                 |
| `GEO_TIER_SYNC_GROUP_ID`        | `geo-discovery-tier-sync`      | service-specific        | Kafka consumer group for `profile.tier_changed`                     |

**Compile-time feature flags:** None. All tuning is runtime via environment variables.

**ScyllaDB execution profile mapping:**
- `ProfileKind::Strict` (`LocalQuorum`) — INSERT, UPDATE (all mutations)
- `ProfileKind::Fast` (`LocalOne` + speculative retry) — SELECT (point reads, cold-start recovery)

---

## 📈 Telemetry, Performance & Metrics

### Execution Prerequisites

- **Async runtime:** Tokio multi-thread (`rt-multi-thread`). The service uses `tokio::join!` for concurrent ScyllaDB and Redis writes; a single-threaded runtime will deadlock under load.
- **CPU architecture:** None — `h3o` is pure Rust with no SIMD or platform-specific code.
- **Memory floor:** ~512 MB JVM-less baseline. Redis memory is external; ScyllaDB is external. The process itself is lean.
- **File descriptor limit:** At minimum `ulimit -n 4096` — one fd per ScyllaDB connection shard, one per Redis connection, several per Kafka consumer.

### Key Operational Metrics (OTel / Prometheus)

| Metric Name                              | Type      | Recommended Alert             | Description                                                     |
|------------------------------------------|-----------|-------------------------------|-----------------------------------------------------------------|
| `geo_discovery_tile_query_duration_ms`   | Histogram | **p99 > 50 ms**               | End-to-end `QueryTile` gRPC handler latency                    |
| `geo_discovery_cache_miss_ratio`         | Gauge     | **> 0.30 for 5 min**          | Fraction of post IDs served from ScyllaDB (not Redis)           |
| `geo_discovery_hot_tile_count`           | Gauge     | **> 50 000**                  | Cardinality of `sg:geo:hot_tiles` — tracks Redis spatial RAM    |
| `geo_discovery_post_indexer_lag_seconds` | Gauge     | **> 30 s**                    | Kafka consumer lag on `post.published`                          |
| `geo_discovery_score_updater_lag_seconds`| Gauge     | **> 10 s**                    | Kafka consumer lag on `engagement.score_updated`                |
| `geo_discovery_tile_pruner_evictions`    | Counter   | —                             | Total cold tile ZSETs evicted (useful for capacity planning)    |
| `geo_discovery_card_serialization_errors`| Counter   | **> 0 in any 1 min window**   | Msgpack failures — indicates schema mismatch or memory issue    |
| `geo_discovery_tile_cells_per_query`     | Histogram | **p95 > 30**                  | H3 cells queried per request — high = viewport too wide         |
| `geo_discovery_scylla_write_errors`      | Counter   | **> 5 / min**                 | ScyllaDB write failures in indexer workers                      |
| `geo_discovery_redis_write_errors`       | Counter   | **> 10 / min**                | Redis write failures (indexer degrades gracefully; still alert) |
| `geo_discovery_tier_sync_lag_seconds`    | Gauge     | **> 60 s**                    | Kafka consumer lag on `profile.tier_changed`                    |

### Memory Budget Reference

| Component            | At 10 M posts | At 100 M posts | Mitigation lever                      |
|----------------------|---------------|----------------|---------------------------------------|
| Redis ZSETs (Top-K)  | ~180 MB       | ~1.8 GB        | Reduce `top_k_cap` per resolution     |
| Redis card cache     | ~500 MB       | ~5 GB          | Raise `GEO_CARD_CACHE_THRESHOLD`      |
| Redis hot_tiles ZSET | ~5 MB         | ~50 MB         | Reduce `GEO_TILE_COLD_THRESHOLD_SECS` |
| **Total Redis**      | **~0.7 GB**   | **~7 GB**      |                                       |

---

## 🛠️ Local Development & Contribution

### Required Local Dependencies

| Dependency   | Version   | Docker Image                    |
|--------------|-----------|---------------------------------|
| ScyllaDB     | 6.x       | `scylladb/scylla:6.2`           |
| Redis        | 7.x       | `redis:7-alpine`                |
| Apache Kafka | 3.x       | `confluentinc/cp-kafka:7.6`     |

### Setup & Build

```bash
# 1. Start infrastructure
docker compose up -d scylla redis kafka

# 2. Wait for ScyllaDB to be ready, then apply migrations in order
cqlsh 127.0.0.1 9042 -f migrations/0001_create_keyspace.cql
cqlsh 127.0.0.1 9042 -f migrations/0002_create_posts_by_tile_table.cql
cqlsh 127.0.0.1 9042 -f migrations/0003_create_map_post_cards_table.cql

# 3. Verify migrations
cqlsh 127.0.0.1 9042 -e "DESCRIBE KEYSPACE geo_discovery;"

# 4. Build
cargo build -p geo-discovery

# 5. Run with local config
SCYLLA_URI=127.0.0.1:9042 \
REDIS_URL=redis://127.0.0.1:6379 \
KAFKA_BROKERS=127.0.0.1:9092 \
cargo run -p geo-discovery
```

### Development Commands

```bash
# Compile check (fast, no linking)
cargo check -p geo-discovery

# Run unit tests (H3 codec + value object invariants)
cargo test -p geo-discovery

# Lint — treat all warnings as errors
cargo clippy -p geo-discovery -- -D warnings

# Format
cargo fmt -p geo-discovery

# Verify proto compilation
cargo build -p geo-discovery   # tonic-build runs in build.rs
```

### Directory Structure

```
crates/services/geo-discovery/
├── build.rs                            # tonic-build proto compilation
├── Cargo.toml
├── migrations/                         # ScyllaDB CQL applied manually (no ORM)
│   ├── 0001_create_keyspace.cql
│   ├── 0002_create_posts_by_tile_table.cql
│   └── 0003_create_map_post_cards_table.cql
├── proto/geo_discovery/v1/
│   ├── enums.proto                     # ZoomBand
│   ├── messages.proto                  # Viewport, MapPostCard, request/response
│   └── service.proto                   # GeoDiscoveryService
└── src/
    ├── lib.rs
    ├── error.rs                        # GeoDiscoveryError + GEO-XXXX codes
    ├── config/mod.rs                   # GeoDiscoveryConfig (env vars)
    ├── domain/
    │   ├── value_object/               # H3Index, H3Resolution, GeoCoordinate, …
    │   └── entity/map_post_card.rs     # The projection struct (Serialize + Deserialize)
    ├── application/
    │   ├── port/                       # SpatialIndex, CardStore, TileRepository traits
    │   ├── command/                    # IndexPostHandler, UpdateViralityWithTilesHandler
    │   └── query/query_tile.rs         # QueryTileHandler (main read path)
    └── infrastructure/
        ├── h3/h3_codec.rs              # viewport_cells() via grid_disk
        ├── cache/
        │   ├── redis_spatial_index.rs  # ZADD_TOPK + ZADD_XX Lua scripts
        │   └── redis_card_store.rs     # msgpack SET + MGET pipeline
        ├── persistence/
        │   └── scylla_tile_repository.rs
        └── worker/
            ├── post_indexer.rs         # Kafka: post.published
            ├── score_updater.rs        # Kafka: engagement.score_updated
            ├── tier_sync.rs            # Kafka: profile.tier_changed
            └── tile_pruner.rs          # Background: PRUNE_COLD_TILES Lua
```

---

## 🚨 Troubleshooting & Runbook (FAQ)

### 1. Map shows no posts despite active content

**Symptom:** `QueryTile` returns `tile_count > 0` but empty `cards`.

**Root causes & diagnosis:**

| Check | Command | Healthy Value |
|---|---|---|
| Redis ZSETs populated? | `redis-cli ZCARD "sg:geo:tile:{h3}:7"` | > 0 |
| Card cache populated? | `redis-cli EXISTS "sg:geo:card:{uuid}"` | 1 |
| ScyllaDB has rows? | `SELECT count(*) FROM geo_discovery.posts_by_tile WHERE h3_index=? AND resolution=7;` | > 0 |
| Indexer lag? | `geo_discovery_post_indexer_lag_seconds` metric | < 30 s |

**Resolution path:**
1. **ZSETs empty, ScyllaDB has rows** → indexer lagging or paused. Check Kafka consumer group: `kafka-consumer-groups.sh --describe --group geo-discovery-post-indexer`. Scale consumer replicas.
2. **ZSETs populated, cards empty** → `GEO_CARD_CACHE_THRESHOLD` too high. Lower to `1.0` temporarily and verify cards appear.
3. **Both empty** → service cold-started after `post.published` events were pruned from Kafka. Publish a test event manually.
4. **Wrong H3 tile computed** → verify the client is sending correct `sw/ne` lat/lng (not inverted). Log the tile indices in `QueryTileHandler` to inspect.

---

### 2. Redis memory growing without bound

**Symptom:** `geo_discovery_hot_tile_count` increasing; Redis `INFO memory → used_memory_human` approaching `maxmemory`.

**Diagnosis:**
```bash
# Check hot tile count
redis-cli ZCARD "sg:geo:hot_tiles"

# Check largest tile ZSETs
redis-cli --scan --pattern "sg:geo:tile:*" | head -20 | xargs -I{} redis-cli ZCARD {}

# Check pruner is evicting
# Look for log: "cold tile ZSETs evicted from Redis"
```

**Resolution path:**
1. **Pruner not running:** Check for log `tile pruner worker started`. If absent, the worker crashed — check `geo_discovery_tile_pruner_evictions` counter for zero.
2. **Cold threshold too high:** Lower `GEO_TILE_COLD_THRESHOLD_SECS` to `900` (15 min) and redeploy.
3. **Top-K cap ineffective:** Verify `ZADD_TOPK_SCRIPT` is executing. Check `geo_discovery_redis_write_errors` counter.
4. **Viral event storm:** A single viral location can fill R9 tiles (1 000-member cap). The cap should prevent runaway growth. If `ZCARD` on a tile exceeds 1 000, the Lua script has a bug — redeploy.
5. **Emergency drain:** `redis-cli FLUSHDB` drops all spatial data. The next tile queries will cold-start from ScyllaDB. Announce maintenance window before executing.

---

### 3. Score updates not reflected on the map

**Symptom:** Viral posts remain stuck at their initial score in tile queries; `engagement.score_updated` events are being published.

**Diagnosis:**
```bash
# Verify ScyllaDB has the updated score
cqlsh -e "SELECT post_id, virality_score FROM geo_discovery.map_post_cards WHERE post_id = <uuid>;"

# Verify Redis ZSET score
redis-cli ZSCORE "sg:geo:tile:<h3>:7" "<post_uuid>"

# Check score updater lag
# Metric: geo_discovery_score_updater_lag_seconds
```

**Root causes & resolution:**

| Situation | Cause | Fix |
|---|---|---|
| ScyllaDB score correct, ZSET score stale | Post was Top-K evicted; `ZADD_XX_SCRIPT` skips absent members | Expected behaviour. Score shows stale until card TTL expires and is refreshed. |
| ScyllaDB score stale too | `ScoreUpdaterWorker` consumer lag | Check Kafka consumer group lag; scale replicas. |
| ScyllaDB card row missing | Post TTL expired before score event arrived | Expected for old posts. No action needed. |
| Score correct in Redis but wrong score displayed | Client is caching the gRPC response | Client-side cache invalidation issue, not server-side. |
