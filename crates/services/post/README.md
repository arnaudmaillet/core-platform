# Post Microservice

Single-point post registry for multi-format media content (Carousel, MainVideo, TextOnly).

---

## Overview

The `post` crate is the canonical source of truth for user-created posts. It enforces content invariants (carousel cardinality, video duration caps, attachment validation), manages a Draft → Published → Deleted lifecycle, and emits Kafka events on every state transition. It has no knowledge of feeds, timelines, or social graphs.

---

## Architecture

```
crates/services/post/
├── migrations/            # CQL DDL — run in order against ScyllaDB
├── proto/post/v1/         # Protobuf definitions (enums, messages, service)
└── src/
    ├── domain/
    │   ├── aggregate/     # Post aggregate root — FSM + invariant enforcement
    │   ├── entity/        # MediaAttachment entity
    │   ├── event/         # DomainEvent enum (PostPublished, PostUpdated, PostDeleted)
    │   └── value_object/  # PostId, ProfileId, PostKind, PostStatus, Caption, CdnUrl, MimeType
    ├── application/
    │   ├── command/       # CreatePost, PublishPost, UpdatePost, DeletePost handlers
    │   ├── query/         # GetPost, ListPostsByProfile handlers
    │   └── port/          # PostRepository and EventPublisher traits
    ├── infrastructure/
    │   ├── persistence/   # ScyllaPostRepository — dual-write, cursor pagination
    │   ├── publisher/     # KafkaEventPublisher
    │   └── grpc/          # PostServiceHandler (gRPC ↔ CQRS bridge) + server impl
    └── error.rs           # PostError (PST-xxxx codes)
```

**Storage design** — two-table wide-column schema:
- `post.posts` — canonical store, partition key `post_id` for O(1) point lookups
- `post.posts_by_profile` — creator-feed index, partition key `profile_id`, clustering `created_at DESC, post_id ASC`

Every write dual-writes to both tables sequentially. Attachments are stored as validated JSON (`text` column) to avoid ScyllaDB UDT migration complexity.

---

## Interface Contract

### gRPC Service — `post.v1.PostService`

| RPC | Input | Output | Purpose |
|-----|-------|--------|---------|
| `CreatePost` | `CreatePostRequest` | `CreatePostResponse` | Create a draft post; PostId pre-generated at gRPC boundary |
| `PublishPost` | `PublishPostRequest` | `CommandResponse` | Transition Draft → Published; emits `post.published` |
| `UpdatePost` | `UpdatePostRequest` | `CommandResponse` | Update caption/attachments; emits `post.updated` |
| `DeletePost` | `DeletePostRequest` | `CommandResponse` | Soft-delete; emits `post.deleted` |
| `GetPost` | `GetPostRequest` | `PostView` | Point lookup by `post_id` |
| `ListPostsByProfile` | `ListPostsByProfileRequest` | `ListPostsByProfileResponse` | Cursor-paginated creator feed |

### Kafka Topics

| Topic | Key | Trigger |
|-------|-----|---------|
| `post.published` | `post_id` | `PublishPost` success |
| `post.updated`   | `post_id` | `UpdatePost` success |
| `post.deleted`   | `post_id` | `DeletePost` success |

---

## Error Codes

| Code | Variant | HTTP | Description |
|------|---------|------|-------------|
| PST-1001 | `PostNotFound` | 404 | Post does not exist |
| PST-1002 | `PostAlreadyPublished` | 409 | Post is already published |
| PST-1003 | `PostAlreadyDeleted` | 409 | Post is already deleted |
| PST-1004 | `NotDraft` | 422 | Can only publish a Draft post |
| PST-1005 | `AuthorMismatch` | 403 | Caller is not the post author |
| PST-2001 | `CarouselTooFewItems` | 422 | Carousel requires ≥ 2 items |
| PST-2002 | `CarouselTooManyItems` | 422 | Carousel exceeds 10 items |
| PST-2003 | `CarouselVideoTooLong` | 422 | Carousel video > 15 s |
| PST-3001 | `MissingVideoThumbnail` | 422 | Video attachment lacks thumbnail |
| PST-3002 | `InvalidMimeType` | 422 | MIME type not in allowlist |
| PST-3003 | `InvalidCdnUrl` | 422 | URL is not a valid HTTPS CDN URL |
| PST-3004 | `InvalidDimensions` | 422 | Attachment dimensions are zero |
| PST-9001 | `InvalidPostId` | 422 | Not a valid UUID |
| PST-9002 | `InvalidProfileId` | 422 | Not a valid UUID |
| PST-9003 | `AttachmentsCorrupted` | 500 | JSON deserialization failure (High) |
| PST-9004 | `DomainViolation` | 422 | Generic invariant breach |

---

## Database Schema

```
Keyspace: post  (NetworkTopologyStrategy, datacenter1: 3, LZ4Compressor)

post.posts                      — canonical; partition by post_id
post.posts_by_profile           — creator index; partition by profile_id
                                   clustering: created_at DESC, post_id ASC
```

Run migrations in order: `0001_create_keyspace.cql` → `0002_create_posts_table.cql` → `0003_create_posts_by_profile_table.cql`.

---

## Business Rules

- **Carousel**: 2–10 items; carousel videos ≤ 15 s each; video items require `thumbnail_url`
- **MainVideo**: single video attachment; requires `thumbnail_url`
- **TextOnly**: zero attachments (caption-only post)
- **Threading**: `parent_id` and `root_id` must both be present or both absent
- **Lifecycle**: Draft → Published (irreversible); Draft or Published → Deleted (soft-delete only)
- **Ownership**: `profile_id` on PublishPost/UpdatePost/DeletePost must match the post author

---

## Deployment

```bash
# Apply CQL migrations (example with cqlsh)
cqlsh -f migrations/0001_create_keyspace.cql
cqlsh -f migrations/0002_create_posts_table.cql
cqlsh -f migrations/0003_create_posts_by_profile_table.cql
```

Dependencies: ScyllaDB cluster (`datacenter1`), Kafka broker.

---

## 🚀 Deployment

Library-only: implements [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `post::service::PostService` (`build` wires the ScyllaDB repository and the
durable Kafka event publisher; `register` adds the gRPC + reflection services;
`health_probes` checks Scylla). The deployable binary is `crates/apps/post-server`:

```rust
use std::net::SocketAddr;
use post::service::PostService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("POST_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50056".to_owned())
        .parse()?;
    service_runtime::serve::<PostService>(addr).await
}
```
