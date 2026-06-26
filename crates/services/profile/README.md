# `profile` — High-throughput public identity layer for hyperscale read traffic

> **Service Card**
>
> | | |
> |---|---|
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |
> | **On-call / escalation** | `<TODO: oncall-rotation>` → `<TODO: escalation-policy>` |
> | **Tier** | **TIER-0** — public read path, fleet-wide identity resolution |
> | **Deployable** | `crates/apps/profile-server` (library crate: `crates/services/profile`) |
> | **Datastores** | ScyllaDB keyspace `profile` · Redis (cache-aside) |
> | **Async** | publishes `profile.v1.events` (incl. `ProfileTierChanged`) · consumes `account.v1.events`, `social-graph.author_tier_changed` |
> | **Upstream callers** | `<TODO: gateway>`, recommendation/bulk-lookup consumers, `geo-discovery` (via events) |
> | **Downstream deps** | ScyllaDB, Redis, Kafka |
> | **SLO** | cache-hit read p99 **< 1 ms** · cache-miss p99 **< 5 ms** |

---

## 🎯 Overview & Service Role

`profile` owns all **public-facing identity metadata**: @handles, display names, bios, avatars, and
profile classification. It is the authoritative read path for any consumer resolving a public identity
— from gateways rendering user cards to recommendation engines doing bulk lookups. It manages a
1-to-N relationship between a private `AccountId` and multiple independent `Profile` aggregates
(personal, professional, brand, bot).

The hard problem it solves is **sub-millisecond reads at hyperscale without cross-service coupling**: a
Redis cache-aside layer over ScyllaDB serves cache hits in < 1 ms, and account lifecycle is ingested
**reactively** via Kafka (`AccountSuspended/Deleted/Activated` → mask/restore) so there is no
synchronous dependency on `account` on the read path.

**Core objectives:** P99 < 1 ms cache hit, < 5 ms cache miss; globally-unique @handles via ScyllaDB
LWT (`IF NOT EXISTS`, no distributed lock); concurrent-write safety via optimistic `IF version = ?`.
**Strict SRP:** zero social-graph logic — follows/friends/feeds belong elsewhere.

---

## 📐 Architecture & Concepts

Hexagonal / DDD, CQRS buses, ScyllaDB durable store, Redis cache-aside, Kafka reactive masking.

```
gRPC ─► ProfileServiceHandler ─► Command bus            Query bus ─► cache.get ─HIT─► return
            │                       │                        │ MISS
            ▼                       ▼                        ▼
   Create/Update/ChangeHandle(LWT)/…              repo.find_by_id ─► cache.set (async)
            │
            ▼
   ScyllaDB: profiles (PK profile_id) · profiles_by_account (PK account_id, CK created_at DESC)
             profile_handles (PK handle — LWT uniqueness index)

   Redis cache-aside: profile:v1:{id} TTL 300s · handle:v1:{handle} TTL 600s · account:profiles:v1:{id} TTL 120s

   Kafka account.v1.events ─► AccountSuspended→HideProfile · AccountDeleted→HideProfile · AccountActivated→RestoreProfile
```

**Cache-key versioning.** All keys carry a `v1:` prefix — bumping the suffix performs a zero-downtime
cache invalidation during schema migrations. **Tombstone reservation:** a deleted handle is blocked
for 30 days via `tombstoned_at`, preventing rapid identity hijacking (`handle_is_available()` enforces
it at the application layer).

> **Invariants** (and where enforced): handle uniqueness via `IF NOT EXISTS` LWT on `profile_handles`;
> optimistic concurrency via `IF version = ?` LWT on `profiles` (→ `PRF-4001`, retryable); status
> transitions (`Active⇄Suspended⇄Hidden→Deleted`, `Deleted` terminal) in the aggregate; `profile_kind`
> immutable after creation.

---

## 📊 Service Level Objectives (SLO)

| SLI | Objective | Window | Measured by |
|---|---|---|---|
| Read p99 — cache hit | **< 1 ms** | 1h | `profile.grpc.request.duration` |
| Read p99 — cache miss | **< 5 ms** | 1h | `profile.scylla.query.duration` |
| gRPC p99 (all RPCs) | < 50 ms (page) | 1h | `profile.grpc.request.duration` |
| Cache hit ratio (`by_id`) | > 80% | 5m | `profile.cache.hit_ratio` |
| Durability | no acked write lost | — | Scylla `LocalQuorum` on writes |

**Error budget:** `<TODO>`. **On burn:** `<TODO>`.

---

## 🔗 Dependencies & Blast Radius

**Downstream — what `profile` needs to function:**

| Dependency | Purpose | If down → | Degradation |
|---|---|---|---|
| ScyllaDB (keyspace `profile`) | durable store | reads + writes fail | **Hard** — `UNAVAILABLE` |
| Redis | cache-aside | cache misses to Scylla | **Soft** — all reads fall through; latency rises |
| Kafka | reactive masking + `profile.tier_changed` | suspend/delete masking stalls | **Soft** — reads/writes unaffected |

**Upstream — who depends on `profile` (blast radius if `profile` fails):**

| Caller | Uses | User-visible impact if `profile` is down |
|---|---|---|
| `<TODO: gateway>` | `GetProfileById/ByHandle` | user cards / identity rendering fail |
| `geo-discovery` | consumes `profile.tier_changed` | map author-tier badges go stale |

> **Critical path?** **Yes** — public identity resolution is in the synchronous render path of most
> user-facing surfaces.

---

## 🔌 Public Interfaces & API Contract

### gRPC — `profile.v1.ProfileService`

```protobuf
service ProfileService {
  // Lifecycle (all return CommandResponse)
  rpc CreateProfile(CreateProfileRequest) returns (CommandResponse);
  rpc UpdateProfile(UpdateProfileRequest) returns (CommandResponse);
  rpc ChangeHandle(ChangeHandleRequest) returns (CommandResponse);   // LWT
  rpc UpdateAvatar(UpdateAvatarRequest) returns (CommandResponse);
  rpc UpdateBanner(UpdateBannerRequest) returns (CommandResponse);
  rpc SetVisibility(SetVisibilityRequest) returns (CommandResponse);
  rpc VerifyProfile(VerifyProfileRequest) returns (CommandResponse);
  rpc HideProfile(HideProfileRequest) returns (CommandResponse);
  rpc RestoreProfile(RestoreProfileRequest) returns (CommandResponse);
  rpc DeleteProfile(DeleteProfileRequest) returns (CommandResponse);
  // Queries
  rpc GetProfileById(GetProfileByIdRequest) returns (ProfileView);
  rpc GetProfileByHandle(GetProfileByHandleRequest) returns (ProfileView);
  rpc ListProfilesByAccount(ListProfilesByAccountRequest) returns (ListProfilesByAccountResponse);
}
```

### Rust ports (hexagonal contract)

```rust
pub trait ProfileRepository: Send + Sync + 'static { /* find_by_id, claim_handle (LWT), save (CAS), … */ }
pub trait ProfileCache:      Send + Sync + 'static { /* get_by_id, set_by_id, invalidate_by_id, … */ }
```

`Profile` aggregate carries `version` (optimistic lock), `handle` (slug `[a-z0-9_.]`, 2–30),
`profile_kind` (immutable), `visibility`, `verified`, `masked_at`/`masking_reason` (set reactively).

### Error contract (`PRF-xxxx`)

| Code | Variant | HTTP | Retryable |
|---|---|---|---|
| PRF-1001 | `ProfileNotFound` | 404 | No |
| PRF-1002 | `HandleAlreadyTaken` | 409 | No |
| PRF-1003 | `HandleReserved` | 409 | No |
| PRF-2001/2002 | `ProfileNotActive` / `InvalidStatusTransition` | 422 | No |
| PRF-4001 | `ConcurrentModification` | 409 | **Yes** |
| PRF-5001 | `ProfileAlreadyVerified` | 409 | No |
| PRF-9001–9010 | domain / parse / validation | 422 | No |
| SDB-* / RDB-* | storage (delegated) | varies | varies |

---

## 📨 Events & Async Contract

**Publishes:**

| Topic | Trigger | Key | Consumers |
|---|---|---|---|
| `profile.v1.events` | every profile lifecycle mutation — `ProfileCreated` / `ProfileUpdated` / `HandleChanged` / `ProfileVerified` / `ProfileHidden` / `ProfileRestored` / `ProfileDeleted` / `ProfileTierChanged` | `profile_id` | `search` (profile indexing), `post` (author-tier denormalization) |

> **Wire contract:** one versioned topic, internally tagged on `type` (the moderation-service convention), keyed by `profile_id` for per-profile ordering. Events are **thin** (ids + timestamps, no display content) — a consumer that needs the full profile hydrates it via `GetProfileById`. Each command handler drains the aggregate's pending events and publishes them **after** the durable write (durable-first; a no-op publisher backs broker-free composition).

**Consumes:**

| Topic | Consumer group | Purpose | On poison/exhaustion |
|---|---|---|---|
| `account.v1.events` | `profile-account-events` | `AccountSuspended/Deleted` → `HideProfile`; `AccountActivated` → `RestoreProfile`; unknown kinds = no-op commit | DLQ `account.v1.events.dlq` |
| `social-graph.author_tier_changed` | `profile-author-tier` | denormalize the author tier onto the profile (`SetProfileTier`) → re-emit on `profile.v1.events` (`ProfileTierChanged`); idempotent on unchanged tier | DLQ `social-graph.author_tier_changed.dlq` |

> **Runtime contract (mandatory):** the account-event consumer runs under `run_consumer` — manual
> commit after success (`enable_auto_commit=false`), bounded retry with backoff + jitter, DLQ on
> exhaustion/poison. Cache errors are always treated as misses; cache-set failures are logged, never
> surfaced.

---

## 🌩️ Failure Modes & Degradation

| Failure | Symptom | Service behavior | Operator action |
|---|---|---|---|
| ScyllaDB unavailable | reads + writes fail | **Hard** — `UNAVAILABLE` | check Scylla cluster / DC |
| Redis unavailable / cold | latency rises | **Soft** — all reads fall through to Scylla (Fast profile) | verify cache hit ratio; usually self-heals |
| Handle LWT race | `PRF-1002` to the losing writer | correct serialization at storage layer | none — surface to user to pick another handle |
| Account-event consumer lag | profiles not masked on suspend/delete | retries within budget; offset uncommitted | check consumer lag; re-dispatch masking if needed |

**Backpressure & limits.** Writes use the Scylla **Strict** profile (`LocalQuorum`); reads use **Fast**
(`LocalOne` + speculative retry firing 1 extra request after 20 ms) to bound tail latency.

---

## 📦 Integration & Usage

```toml
[dependencies]
profile = { path = "crates/services/profile" }
```

Library-only. Implements [`service_runtime::Service`](../../platform/service-runtime/README.md) as
`profile::service::ProfileService`. `build(infra)` reads cache-TTL profiles from the `[cache]` section
of `infrastructure.toml`, assembles repository/cache/CQRS buses, and **self-spawns the supervised
account-event consumer**; `register` adds the gRPC + reflection services; `health_probes` checks
Scylla/Redis. Profile is the canonical *infra-consuming* service. The integration harness drives
`App::build` directly, so the wired graph under test is the one that ships.

### Bootstrap (`crates/apps/profile-server`)

```rust
use std::net::SocketAddr;
use profile::service::ProfileService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("PROFILE_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50052".to_owned())
        .parse()?;
    service_runtime::serve::<ProfileService>(addr).await
}
```

---

## ⚙️ Configuration & Runtime Environment

### Inherited infrastructure variables (key subset)

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_CONTACT_POINTS` | **Yes** | `127.0.0.1:9042` | ScyllaDB contact points. |
| `SCYLLA_LOCAL_DC` | **Yes** | `datacenter1` | DC for token-aware routing. |
| `SCYLLA_KEYSPACE` | No | — | Leave unset if queries fully-qualify table names (recommended). |
| `REDIS_URL` | **Yes** | `redis://127.0.0.1:6379` | Redis connection URL. |
| `KAFKA_BROKERS` | **Yes** | `127.0.0.1:9092` | Kafka brokers. |
| `KAFKA_CONSUMER_GROUP` | No | `profile-service` | account-event consumer group. |
| `PROFILE_GRPC_ADDR` | No | `0.0.0.0:50052` | gRPC bind address. |

> Full `SCYLLA_*` / `REDIS_*` / `KAFKA_*` tuning is documented in the shared storage/transport crates.
> The `[cache]` TTL profiles are consumed from `infrastructure.toml`, not env.

### Compile-time features
- `build.rs` compiles `proto/profile/v1/*.proto` and emits the reflection descriptor set.

---

## 🚀 Deployment, Migrations & Rollback

- **Migrations:** `crates/services/profile/migrations/000{1..4}_*.cql` against the `profile` keyspace,
  applied **before** first start.
- **Cache version bump:** to invalidate cache fleet-wide during a schema change, bump the `v1:` key
  prefix — no flush, no downtime.
- **Rollout/Rollback:** `<TODO: strategy>`; stateless service, safe to roll.

---

## 📈 Telemetry, Performance & Metrics

- **Runtime:** Tokio multi-thread. Requires a reachable contact point in `SCYLLA_LOCAL_DC` at startup;
  Redis unavailability degrades gracefully.

| Signal | Why it matters | Suggested alert |
|---|---|---|
| `profile.grpc.request.duration` p99 | read-path SLO | > 50 ms for 2m ⇒ page |
| `profile.cache.hit_ratio` (`by_id`) | cache health | < 70% ⇒ investigate TTL/eviction |
| `profile.handle.claim.conflict_total` | LWT contention | > 10/min ⇒ possible hijack attempt |
| `profile.concurrent_modification_total` | write hot-spots | sustained > 0 ⇒ retry-storm risk |
| `profile.kafka.consumer.lag` | masking freshness | > 5 000 ⇒ scale consumers |

---

## 🛠️ Local Development

```bash
docker compose up -d                          # ScyllaDB, Redis, Kafka, OTel collector
cargo build -p profile && cargo clippy -p profile -- -D warnings
cargo test  -p profile                        # add --features integration for live-infra tests
for f in crates/services/profile/migrations/*.cql; do cqlsh -f "$f"; done
```

---

## 🚨 Troubleshooting & Runbook

> Format: **symptom → root cause → mitigation.**

**1. `PRF-1002 HandleAlreadyTaken` though the handle looks free.**
Root cause: a concurrent `CreateProfile`/`ChangeHandle` won the `IF NOT EXISTS` LWT race. Mitigation:
correct behavior — the LWT serialized the conflict at storage; the client should prompt for a different
handle. No manual intervention.

**2. Profile cache shows stale data after an update.**
Root cause: a parallel read repopulated the cache from an in-flight Scylla query that returned the old
version pre-quorum. Mitigation: the 300 s TTL bounds staleness; for immediate consistency
`DEL profile:v1:{id}` and `DEL handle:v1:{handle}`, then re-read to repopulate.

**3. Account events stop masking profiles after a Scylla node failure.**
Root cause: a `HideProfile`/`RestoreProfile` handler returned `Storage`; the message retries/dead-letters.
Mitigation: watch `profile.kafka.consumer.lag` and `account.v1.events.dlq`; for stuck profiles, query
Scylla by `account_id` and re-dispatch masking via admin tooling.
