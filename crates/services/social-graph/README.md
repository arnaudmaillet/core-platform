# social-graph

Production-grade social graph microservice implementing directional follow/block relationships between opaque `ProfileId` (UUIDv7) primitives.

## Bounded Context

This service is the strict owner of **who follows whom** and **who blocks whom**.

| In scope | Out of scope |
|---|---|
| Follow, Unfollow, Block, Unblock | Profile metadata (handles, bios, avatars) |
| Follower/following counts (Redis) | Account management |
| Block-gate enforcement | Timeline feed construction |
| Mutual-follow derivation (Redis) | Notification delivery |
| Kafka event emission | Post/content ownership |

Profiles are treated as opaque UUIDv7 identifiers. This service never imports `services/profile` or `services/account`.

---

## Architecture

```
┌───────────────────────────────────────────────────┐
│  gRPC (Tonic)  SocialGraphService                 │
│  ─────────────────────────────────────────────── │
│  Commands: Follow / Unfollow / Block / Unblock     │
│  Queries:  GetRelationStatus / ListFollowers /     │
│            ListFollowing / ListBlocks              │
└───────────────────────┬───────────────────────────┘
                        │ CQRS bus
        ┌───────────────┼───────────────────┐
        ▼               ▼                   ▼
  Command Handlers  Query Handlers    EventPublisher
        │               │                   │
        ▼               ▼                   ▼
  SocialGraphRepository  SocialGraphCache  KafkaProducerHandle
        │                      │
        ▼                      ▼
   ScyllaDB (4 tables)    Redis (sets + counters)
```

### Layer responsibilities

| Layer | Responsibility |
|---|---|
| `domain/aggregate/relation.rs` | Invariant enforcement (self-interaction, block-gate, sever-on-block) |
| `application/command/` | Orchestrate repo + cache + publisher; no domain logic |
| `application/query/` | Assemble read-model from ScyllaDB + Redis |
| `infrastructure/persistence/` | ScyllaDB queries (strict/fast profiles) |
| `infrastructure/cache/` | Redis Set (following, blocks) + INCR counters |
| `infrastructure/publisher/` | Kafka JSON envelope per domain event |
| `infrastructure/grpc/` | Proto↔domain mapping; error→Status translation |

---

## ScyllaDB Schema

### Keyspace

```cql
CREATE KEYSPACE social_graph
    WITH replication = {'class': 'NetworkTopologyStrategy', 'datacenter1': 3};
```

### Table Overview

| Table | Partition Key | Clustering Key | Purpose |
|---|---|---|---|
| `followers` | `followee_id` | `followed_at DESC, follower_id ASC` | Fan-in: who follows X? |
| `following` | `follower_id` | `followed_at DESC, followee_id ASC` | Fan-out: who does X follow? |
| `follow_status` | `follower_id` | `followee_id ASC` | Point-lookup + `followed_at` for DELETE |
| `blocks` | `blocker_id` | `blockee_id ASC` | Block point-lookup + list |

#### Why `follow_status`?

ScyllaDB DELETE requires the **full clustering key**. The `followers` and `following` tables include `followed_at` in their clustering key, so unfollowing and block-severing operations must know this timestamp before issuing a DELETE. `follow_status` stores it as a regular column, avoiding any read-before-write on the adjacency list tables.

#### Why no `blocked_by` mirror?

The block-gate check requires two point-lookups: `blocks(A,B)` and `blocks(B,A)`. Both are O(1) on the **same table** with swapped arguments — no mirror needed.

---

## Redis Cache Strategy

| Key | Type | Written by | Read by |
|---|---|---|---|
| `sg:following:v1:{id}` | Set | Follow / Unfollow / Block | Kafka downstream engines (SINTER for friend detection) |
| `sg:blocks:v1:{id}` | Set | Block / Unblock | (future: fast gate check in read path) |
| `sg:followers_count:v1:{id}` | String | Follow / Unfollow / Block | `GetRelationStatus` query |
| `sg:following_count:v1:{id}` | String | Follow / Unfollow / Block | `GetRelationStatus` query |

### Why Sets for `following` but counters for `followers`?

Outbound follows are bounded for all users (max tens of thousands). Inbound follows are unbounded for celebrity profiles (millions). Materializing the full inbound set would exhaust Redis memory. A INCR/DECR counter satisfies count reads with O(1) space.

### Mutual-follow (friendship) derivation

```
IsFriend(A, B) =
    SISMEMBER sg:following:v1:{A} {B}   -- A follows B
    AND
    SISMEMBER sg:following:v1:{B} {A}   -- B follows A
```

No dedicated `friends` table exists. Derivation is computed via Redis SISMEMBER (O(1)) or SINTER for batch mutual-follows. This prevents dual-write desynchronization during unfollow/block events.

---

## Domain Invariants

| Invariant | Enforced in |
|---|---|
| A profile cannot follow itself | `FollowProfileHandler` (pre-aggregate check) |
| A profile cannot block itself | `BlockProfileHandler` (pre-aggregate check) |
| Block-gate: follow rejected if any block exists (either direction) | `Relation::follow()` |
| Re-follow rejected if already following | `Relation::follow()` |
| Re-block rejected if already blocked | `Relation::block()` |
| Block severs existing follows in both directions | `Relation::block()` → returns `SeveredFollows` |
| Unblock does not restore severed follows | Intentional; user must re-follow explicitly |

---

## Kafka Events

| Event | Topic | Key | Subscribers |
|---|---|---|---|
| `ProfileFollowed` | `social-graph.followed` | `{actor}:{target}` | Timeline fan-out, notification service |
| `ProfileUnfollowed` | `social-graph.unfollowed` | `{actor}:{target}` | Timeline pruning |
| `ProfileBlocked` | `social-graph.blocked` | `{actor}:{target}` | Content filtering, notification suppression |
| `ProfileUnblocked` | — | — | Not published (no downstream fan-out needed) |

---

## Error Catalogue

| Code | Variant | HTTP | Retryable |
|---|---|---|---|
| SGR-1001 | AlreadyFollowing | 409 | No |
| SGR-1002 | NotFollowing | 422 | No |
| SGR-1003 | AlreadyBlocked | 409 | No |
| SGR-1004 | NotBlocked | 422 | No |
| SGR-2001 | SelfInteraction | 422 | No |
| SGR-2002 | BlockGateDenied | 422 | No |
| SGR-9001 | DomainViolation | 422 | No |
| SGR-9002 | InvalidProfileId | 422 | No |
| SDB-\* | Storage (ScyllaDB) | var | var |
| RDB-\* | Cache (Redis) | 500 | var |
| VAL-\* | Validation | 422 | No |

---

## gRPC Service Interface

```protobuf
service SocialGraphService {
    // Commands
    rpc Follow(FollowRequest)       returns (CommandResponse);
    rpc Unfollow(UnfollowRequest)   returns (CommandResponse);
    rpc Block(BlockRequest)         returns (CommandResponse);
    rpc Unblock(UnblockRequest)     returns (CommandResponse);

    // Queries
    rpc GetRelationStatus(GetRelationStatusRequest) returns (RelationStatusView);
    rpc ListFollowers(ListFollowersRequest)         returns (ListFollowersResponse);
    rpc ListFollowing(ListFollowingRequest)         returns (ListFollowingResponse);
    rpc ListBlocks(ListBlocksRequest)               returns (ListBlocksResponse);
}
```

`RelationStatus` enum values (from actor's perspective):

| Value | Meaning |
|---|---|
| `NONE` | No relationship |
| `FOLLOWING` | Actor follows target only |
| `FOLLOWED_BY` | Target follows actor only |
| `MUTUAL` | Both follow each other (implicit friendship) |
| `BLOCKING` | Actor has blocked target |
| `BLOCKED_BY` | Target has blocked actor |

---

## Running Migrations

```bash
# Apply in order using any CQL client (cqlsh, astra, etc.)
cqlsh -f migrations/0001_create_keyspace.cql
cqlsh -f migrations/0002_create_followers_table.cql
cqlsh -f migrations/0003_create_following_table.cql
cqlsh -f migrations/0004_create_follow_status_table.cql
cqlsh -f migrations/0005_create_blocks_table.cql
```

---

## 🚀 Deployment

Library-only: implements [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `social_graph::service::SocialGraphService` (`build` wires the ScyllaDB
repository, Redis cache, and the durable Kafka event publisher; `register` adds
the gRPC + reflection services; `health_probes` checks Scylla/Redis). The
deployable binary is `crates/apps/social-graph-server`:

```rust
use std::net::SocketAddr;
use social_graph::service::SocialGraphService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("SOCIAL_GRAPH_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50053".to_owned())
        .parse()?;
    service_runtime::serve::<SocialGraphService>(addr).await
}
```
