# profile — High-throughput public identity layer for hyperscale read traffic

## 🎯 Overview & Service Role

The **Profile** microservice owns all public-facing identity metadata on the platform: @handles, display names, bios, avatars, and profile classification. It is the authoritative read path for any consumer that needs to resolve a public identity — from API gateways rendering user cards to recommendation engines performing bulk lookups.

**What it does:**
- Manages a 1-to-N relationship between a private `AccountId` and multiple independent `Profile` aggregates (personal, professional, brand, bot).
- Provides sub-millisecond read latency under hyperscale traffic via a Redis cache-aside layer backed by ScyllaDB.
- Reactively masks profiles via a Kafka consumer that ingests account lifecycle events (`AccountSuspended`, `AccountDeleted`, `AccountActivated`) — no cross-service database coupling.
- Enforces globally unique @handles with a 30-day tombstone reservation on deletion, preventing rapid identity hijacking.

**Strict SRP boundary:** This service contains **zero** social graph logic. Follower counts, friend relationships, and feed relevance belong to a separate bounded context.

**Core technical objectives:**
- **P99 read latency < 1 ms** for cache-hit paths (Redis `GET`).
- **P99 read latency < 5 ms** for cache-miss paths (ScyllaDB `LocalOne` + speculative execution).
- **Handle uniqueness** enforced via ScyllaDB LWT (`IF NOT EXISTS`) — no distributed lock required.
- **Concurrent write safety** via optimistic locking (`IF version = ?`) on every profile update.

---

## 📐 Architecture & Concepts

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          gRPC clients (gateway, services)                   │
└───────────────────────────────────┬─────────────────────────────────────────┘
                                    │ tonic / ProfileService
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         ProfileServiceHandler                               │
│              (CommandBus dispatcher | QueryBus dispatcher)                  │
└───────────┬────────────────────────────────────────┬────────────────────────┘
            │ Commands                               │ Queries
            ▼                                       ▼
┌───────────────────────┐              ┌────────────────────────────────────┐
│  CQRS Command Bus     │              │  CQRS Query Bus                    │
│  - CreateProfile      │              │  1. cache.get_by_id()  ─► HIT ──►  │
│  - UpdateProfile      │              │          │ MISS                    │
│  - ChangeHandle  (LWT)│              │          ▼                         │
│  - UpdateAvatar       │              │  2. repo.find_by_id()              │
│  - UpdateBanner       │              │          │                         │
│  - SetVisibility      │              │  3. cache.set_by_id() (async)      │
│  - VerifyProfile      │              └────────────────────────────────────┘
│  - HideProfile        │
│  - RestoreProfile     │        ┌─── Redis (cache-aside) ────────────────────┐
│  - DeleteProfile      │        │  profile:v1:{id}          TTL 300 s        │
└──────────┬────────────┘        │  handle:v1:{handle}       TTL 600 s        │
           │                     │  account:profiles:v1:{id} TTL 120 s        │
           ▼                     └────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────────────────┐
│                ScyllaDB  (token-aware, DC-local routing)                    │
│                                                                             │
│  profile.profiles            (PK: profile_id)          — full aggregate    │
│  profile.profiles_by_account (PK: account_id, CK: created_at DESC)        │
│  profile.profile_handles     (PK: handle)              — handle index      │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│  Kafka topic: account.v1.events        (account event consumer)             │
│  AccountSuspended  ──► HideProfileCommand  (masking_reason=account_suspended)│
│  AccountDeleted    ──► HideProfileCommand  (masking_reason=account_deleted)  │
│  AccountActivated  ──► RestoreProfileCommand                                │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Resilience Guarantees & High-Load Behavior

| Concern | Mechanism |
|---|---|
| **Cache failure** | All cache errors are treated as misses — the service always falls back to ScyllaDB. Cache set errors are logged but never surfaced to the caller. |
| **ScyllaDB read latency** | `Fast` execution profile: `LocalOne` consistency + speculative retry (fires 1 extra request after 20 ms). Limits tail latency without sacrificing correctness on reads. |
| **ScyllaDB write safety** | `Strict` execution profile: `LocalQuorum` consistency. Mutations are rejected if quorum is unavailable. |
| **Handle race conditions** | `IF NOT EXISTS` LWT on `profile_handles` table. Only one writer can claim a handle even under concurrent create storms. |
| **Optimistic write conflicts** | `IF version = ?` LWT on `profile.profiles`. Returns `PRF-4001` (retryable) to the caller. |
| **Kafka consumer failures** | Per-message error handling: bad messages are logged and skipped; the consumer loop never panics. Stream exhaustion is logged as a warning. |
| **Tombstone reservation** | Deleted handles are blocked for 30 days via `tombstoned_at` timestamp; `handle_is_available()` enforces this at the application layer. |
| **Cache key versioning** | All keys prefixed with `v1:` — bumping the version suffix allows zero-downtime cache invalidation during schema migrations. |

---

## 🔌 Public Interfaces & API Contract

### gRPC Service — `profile.v1.ProfileService`

```protobuf
service ProfileService {
    // ── Lifecycle ─────────────────────────────────────────────────────────────
    rpc CreateProfile(CreateProfileRequest)   returns (CommandResponse);
    rpc UpdateProfile(UpdateProfileRequest)   returns (CommandResponse);
    rpc ChangeHandle(ChangeHandleRequest)     returns (CommandResponse);
    rpc UpdateAvatar(UpdateAvatarRequest)     returns (CommandResponse);
    rpc UpdateBanner(UpdateBannerRequest)     returns (CommandResponse);
    rpc SetVisibility(SetVisibilityRequest)   returns (CommandResponse);
    rpc VerifyProfile(VerifyProfileRequest)   returns (CommandResponse);
    rpc HideProfile(HideProfileRequest)       returns (CommandResponse);
    rpc RestoreProfile(RestoreProfileRequest) returns (CommandResponse);
    rpc DeleteProfile(DeleteProfileRequest)   returns (CommandResponse);
    // ── Queries ───────────────────────────────────────────────────────────────
    rpc GetProfileById(GetProfileByIdRequest)              returns (ProfileView);
    rpc GetProfileByHandle(GetProfileByHandleRequest)      returns (ProfileView);
    rpc ListProfilesByAccount(ListProfilesByAccountRequest) returns (ListProfilesByAccountResponse);
}
```

### Core Domain Types (Rust)

```rust
// Aggregate root
pub struct Profile {
    id: ProfileId,                         // UUID v7
    account_id: AccountId,                 // UUID v7, immutable
    version: i64,                          // optimistic lock counter
    handle: Handle,                        // validated slug [a-z0-9_.], 2-30 chars
    display_name: DisplayName,             // max 100 chars
    bio: Option<Bio>,                      // max 500 chars
    avatar_url: Option<AvatarUrl>,         // HTTPS CDN URL
    banner_url: Option<BannerUrl>,         // HTTPS CDN URL
    website_url: Option<WebsiteUrl>,       // HTTPS URL
    custom_links: Vec<ProfileLink>,        // max 5 entries
    profile_kind: ProfileKind,             // Personal | Professional | Brand | Bot (immutable)
    visibility: ProfileVisibility,         // Public | Private
    verified: bool,
    verification_kind: Option<VerificationKind>, // Official | Notable | Business
    locale: Locale,                        // BCP-47
    timezone: Option<String>,              // IANA tz
    status: ProfileStatus,                 // Active | Suspended | Hidden | Deleted
    masked_at: Option<DateTime<Utc>>,      // set by reactive account events
    masking_reason: Option<MaskingReason>, // AccountSuspended | AccountDeleted | ContentPolicy
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

// Port traits (injected at the composition root)
pub trait ProfileRepository: Send + Sync + 'static { /* ... */ }
pub trait ProfileCache:      Send + Sync + 'static { /* ... */ }
```

### Status Transition Matrix

```
Active  ──► Suspended | Hidden | Deleted
Suspended ──► Active | Hidden | Deleted
Hidden ──► Active | Suspended | Deleted
Deleted  — terminal, no outgoing transitions
```

### Error Catalogue (PRF-xxxx)

| Code | Variant | HTTP | Retryable |
|---|---|---|---|
| PRF-1001 | `ProfileNotFound` | 404 | No |
| PRF-1002 | `HandleAlreadyTaken` | 409 | No |
| PRF-1003 | `HandleReserved` | 409 | No |
| PRF-2001 | `ProfileNotActive` | 422 | No |
| PRF-2002 | `InvalidStatusTransition` | 422 | No |
| PRF-4001 | `ConcurrentModification` | 409 | **Yes** |
| PRF-5001 | `ProfileAlreadyVerified` | 409 | No |
| PRF-9001 | `DomainViolation` | 422 | No |
| PRF-9002–9010 | Parse/validation errors | 422 | No |
| SDB-* | ScyllaDB storage (delegated) | varies | varies |
| RDB-* | Redis cache (delegated) | 500 | varies |

---

## 📦 Integration & Usage

### Cargo.toml

```toml
[dependencies]
profile = { path = "crates/services/profile" }
```

### Bootstrap Pattern

```rust
use std::sync::Arc;
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder};
use redis_storage::{RedisClientBuilder, RedisConfig};
use transport::kafka::{KafkaConsumerConfig, KafkaConsumerBuilder};
use cqrs::{InMemoryCommandBus, InMemoryQueryBus};

use profile::application::command::*;
use profile::application::query::*;
use profile::application::port::{ProfileRepository, ProfileCache};
use profile::infrastructure::persistence::ScyllaProfileRepository;
use profile::infrastructure::cache::RedisProfileCache;
use profile::infrastructure::consumer::run_account_event_consumer;
use profile::infrastructure::grpc::handler::ProfileServiceHandler;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ── Storage ───────────────────────────────────────────────────────────────
    let scylla_client = Arc::new(
        ScyllaSessionBuilder::new(ScyllaConfig::from_env())
            .build()
            .await?,
    );
    let redis_client = Arc::new(
        RedisClientBuilder::new(RedisConfig::from_env())
            .build()
            .await?,
    );

    // ── Adapters ──────────────────────────────────────────────────────────────
    let repo: Arc<dyn ProfileRepository> =
        Arc::new(ScyllaProfileRepository::new(Arc::clone(&scylla_client)));
    let cache: Arc<dyn ProfileCache> =
        Arc::new(RedisProfileCache::new(Arc::clone(&redis_client)));

    // ── CQRS buses ────────────────────────────────────────────────────────────
    let mut cmd_bus = InMemoryCommandBus::new();
    cmd_bus.register(CreateProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(UpdateProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(ChangeHandleHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(UpdateAvatarHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(UpdateBannerHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(SetVisibilityHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(VerifyProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(HideProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(RestoreProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    cmd_bus.register(DeleteProfileHandler::new(Arc::clone(&repo), Arc::clone(&cache)));

    let mut qry_bus = InMemoryQueryBus::new();
    qry_bus.register(GetProfileByIdHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    qry_bus.register(GetProfileByHandleHandler::new(Arc::clone(&repo), Arc::clone(&cache)));
    qry_bus.register(ListProfilesByAccountHandler::new(Arc::clone(&repo)));

    // ── gRPC server ───────────────────────────────────────────────────────────
    let handler = ProfileServiceHandler::new(cmd_bus.clone(), qry_bus);
    let grpc_addr = std::env::var("PROFILE_GRPC_HOST")
        .unwrap_or_else(|_| "0.0.0.0:50052".into())
        .parse()?;

    // ── Kafka account event consumer ──────────────────────────────────────────
    let kafka_consumer = KafkaConsumerBuilder::new(KafkaConsumerConfig::from_env())
        .subscribe(&["account.v1.events"])
        .build()?;
    tokio::spawn(run_account_event_consumer(kafka_consumer, cmd_bus));

    // ── Start server ──────────────────────────────────────────────────────────
    tonic::transport::Server::builder()
        .add_service(profile::infrastructure::grpc::handler::ProfileServiceServer::new(handler))
        .serve(grpc_addr)
        .await?;

    Ok(())
}
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_CONTACT_POINTS` | Yes | `127.0.0.1:9042` | Comma-separated list of ScyllaDB `host:port` contact points for bootstrap discovery. |
| `SCYLLA_LOCAL_DC` | Yes | `datacenter1` | Datacenter name this process is co-located with. Must match `system.local`. Drives token-aware, DC-local routing. |
| `SCYLLA_KEYSPACE` | No | _(none)_ | Default keyspace sent on session open. Leave unset if queries fully-qualify table names (recommended). |
| `SCYLLA_USERNAME` | No | _(none)_ | CQL authentication username. |
| `SCYLLA_PASSWORD` | No | _(none)_ | CQL authentication password. |
| `SCYLLA_COMPRESSION` | No | `lz4` | Wire-protocol compression: `lz4` \| `snappy` \| `none`. |
| `SCYLLA_CONNECT_TIMEOUT_SECS` | No | `5` | TCP+CQL handshake timeout in seconds. |
| `SCYLLA_REQUEST_TIMEOUT_SECS` | No | `5` | Per-request timeout in seconds. |
| `SCYLLA_STATEMENT_CACHE_CAPACITY` | No | `256` | Prepared-statement LRU cache size (entries). |
| `REDIS_URL` | Yes | `redis://127.0.0.1:6379` | Redis connection URL (single-node, sentinel, or cluster). |
| `REDIS_POOL_SIZE` | No | `8` | Number of Redis connections in the pool. |
| `REDIS_CONNECT_TIMEOUT_SECS` | No | `3` | TCP connection timeout in seconds. |
| `KAFKA_BROKERS` | Yes | `127.0.0.1:9092` | Comma-separated Kafka broker addresses. |
| `KAFKA_CONSUMER_GROUP` | No | `profile-service` | Kafka consumer group ID. |
| `KAFKA_AUTO_OFFSET_RESET` | No | `earliest` | Offset reset policy for new consumer groups. |
| `PROFILE_GRPC_HOST` | No | `0.0.0.0:50052` | Bind address for the gRPC server. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | _(none)_ | OpenTelemetry collector endpoint (traces + metrics). |
| `OTEL_SERVICE_NAME` | No | `profile` | Service name reported in OTel spans and metrics. |
| `RUST_LOG` | No | `info` | Tracing log filter (e.g. `profile=debug,scylla=warn`). |

---

## 📈 Telemetry, Performance & Metrics

### Runtime Prerequisites

- **Async runtime:** Tokio multi-thread (`rt-multi-thread` feature).
- **ScyllaDB:** Requires at least one reachable contact point in `SCYLLA_LOCAL_DC` at startup.
- **Redis:** Requires a reachable Redis node. Cache unavailability degrades gracefully (all reads fall through to ScyllaDB).

### Key OTel Metrics

| Metric | Type | Labels | Alert Threshold |
|---|---|---|---|
| `profile.grpc.request.duration` | Histogram | `rpc`, `status_code` | P99 > 50 ms → page |
| `profile.cache.hit_ratio` | Gauge | `namespace` (by_id / by_handle) | < 80% → investigate TTL or eviction |
| `profile.scylla.query.duration` | Histogram | `operation` | P99 > 10 ms → page |
| `profile.handle.claim.conflict_total` | Counter | — | Spike > 10/min → possible attack |
| `profile.concurrent_modification_total` | Counter | — | Sustained > 0 → retry-storm risk |
| `profile.kafka.consumer.lag` | Gauge | `partition` | > 1 000 → consumer falling behind |
| `profile.kafka.event.processed_total` | Counter | `event_kind` | — |
| `profile.cache.invalidation_total` | Counter | `reason` | — |

### Recommended Production Alerts

- **P99 gRPC latency > 50 ms** sustained for > 2 minutes → page on-call.
- **Cache hit ratio < 70%** for `by_id` namespace → investigate Redis capacity or TTL configuration.
- **`PRF-4001` (ConcurrentModification) rate > 5/min** → check for write hot-spots on popular profiles.
- **Kafka consumer lag > 5 000 messages** → scale consumer replicas or investigate downstream ScyllaDB pressure.

---

## 🛠️ Local Development & Contribution

### Prerequisites

```bash
docker compose up -d   # starts ScyllaDB, Redis, Kafka, and an OTel collector
```

### Build & Lint

```bash
# from workspace root
cargo build -p profile
cargo clippy -p profile -- -D warnings
cargo fmt --package profile -- --check
```

### Unit Tests

```bash
cargo test -p profile
```

### Integration Tests

Integration tests require live infrastructure (ScyllaDB + Redis). Apply the CQL migrations first:

```bash
# Apply CQL DDL (requires cqlsh or equivalent)
cqlsh -f crates/services/profile/migrations/0001_create_keyspace.cql
cqlsh -f crates/services/profile/migrations/0002_create_profiles_table.cql
cqlsh -f crates/services/profile/migrations/0003_create_profiles_by_account_table.cql
cqlsh -f crates/services/profile/migrations/0004_create_profile_handles_table.cql

# Run integration tests
cargo test -p profile --features integration
```

### Proto Codegen

Proto files are compiled automatically by `build.rs` during `cargo build`. No manual codegen step is needed.

---

## 🚨 Troubleshooting & Runbook

### 1. `PRF-1002 HandleAlreadyTaken` on `CreateProfile` even though the handle appears free

**Root cause:** A concurrent `CreateProfile` or `ChangeHandle` request won the ScyllaDB LWT race (`IF NOT EXISTS`). The `claim_handle` call in the handler returned `applied = false`.

**Mitigation:** This is correct behavior — the LWT serialized the conflict at the storage layer. The client should surface the error to the user and prompt them to choose a different handle. No manual intervention is needed.

### 2. Profile cache shows stale data after an update

**Root cause:** The cache invalidation step (`cache.invalidate_by_id`) completed, but a parallel read populated the cache from an in-flight ScyllaDB query that returned the old version before the write quorum was achieved.

**Mitigation:** Redis TTLs (300 s) bound the staleness window. For immediate consistency, call `DEL profile:v1:{id}` and `DEL handle:v1:{handle}` manually in redis-cli, then re-read the profile to trigger a fresh cache population from ScyllaDB.

### 3. Kafka consumer stops processing account events after a ScyllaDB node failure

**Root cause:** The `HideProfileCommand` or `RestoreProfileCommand` handler returned a `Storage` error, which the consumer logged and skipped. The consumer continued to the next message, but the affected profiles were not masked.

**Mitigation:** Monitor `profile.kafka.event.processed_total` vs `profile.kafka.consumer.lag`. If lag is not growing but some profiles remain unmasked, query ScyllaDB directly for the affected `account_id` and dispatch the masking commands manually via the admin gRPC endpoint or a one-shot migration script. Long-term: implement a Kafka dead-letter topic for failed masking events.
