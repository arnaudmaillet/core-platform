# Local backend fleet (for frontend/client testing)

Run the backend on your laptop so a native/mobile gRPC client can hit it directly
on `localhost` — with **real-mode login** through Keycloak. No Kubernetes, no cloud.

## What comes up

| Layer | Containers |
|---|---|
| Datastores | ScyllaDB, Redis (shared), Redpanda (Kafka API), PostgreSQL, MinIO (S3), OpenSearch, OTLP sink |
| Identity | Keycloak (realm `core-platform`, imported from `keycloak/`) |
| Bootstrap (one-shot) | `db-init` (Postgres DBs) → `migrator` (schema) → `scylla-rf1` (RF=1 downshift) → `topic-provisioner` (Kafka topics) → `minio-init` (bucket) |
| Services | account, auth, profile, social-graph, post, comment, engagement, timeline, notification, chat, geo-discovery, counter (server+worker), moderation, media, search, realtime (gateway+dispatcher) |
| Seed | `seed` one-shot — 3 users, mutual follows, ~5 posts each |

`audit` is intentionally **not** included (compliance plane, not needed for frontend dev).

## gRPC ports (published on `localhost`, 1:1 with the prod registry)

| Service | Port | Service | Port |
|---|---|---|---|
| chat | 50051 | media | 50063 |
| profile | 50052 | counter (server) | 50064 |
| social-graph | 50053 | counter (worker) | 50065 |
| geo-discovery | 50054 | realtime gateway (gRPC) | 50066 |
| notification | 50055 | realtime dispatcher | 50067 |
| post | 50056 | timeline | 50070 |
| comment | 50057 | auth JWKS (HTTP) | 8081 |
| engagement | 50058 | realtime WS | 8443 |
| account | 50059 | Keycloak (host) | 8085 |
| auth (gRPC) | 50060 | MinIO API / console | 9000 / 9001 |
| moderation | 50061 | OpenSearch | 9200 |
| search | 50062 | Kafka (host) | 19092 |

Datastores also exposed: Postgres `:5432`, Redis `:6379`, Scylla `:9042`.

## Usage

```bash
# 0. Generate the local-only ES256 signing key auth needs (once; writes to
#    local-dev/secrets/, which is gitignored — nothing sensitive is committed).
bash local-dev/generate-secrets.sh

# From the repo root. First build compiles ~20 Rust release binaries — build
# SERIALLY (see below); the cargo-chef dep layer is cooked once and reused.
docker compose -f local-dev/docker-compose.fleet.yml up -d

# Seed dev data (idempotent; runs in-network so the Keycloak issuer matches auth)
docker compose -f local-dev/docker-compose.fleet.yml run --rm --no-deps seed

# Tear down (keep data) / wipe volumes
docker compose -f local-dev/docker-compose.fleet.yml down          # -v to wipe
```

### Building (memory-safe)

A parallel build of all images OOM-kills Docker (release `rustc` is heavy). Build
one image at a time; the first image cooks the shared dependency layer, the rest
reuse it:

```bash
for s in $(docker compose -f local-dev/docker-compose.fleet.yml config --services); do
  docker compose -f local-dev/docker-compose.fleet.yml build "$s"
done
```

## Seeded dev users

Three Keycloak users (password **`password`** for all), each with an Active account,
a profile, mutual follows, and 5 published posts:

| username | handle | email |
|---|---|---|
| alice | @alice | alice@dev.local |
| bob | @bob | bob@dev.local |
| carol | @carol | carol@dev.local |

**Real login** (what iOS does): `auth.v1.AuthService/Login` with `grant_type: PASSWORD`
and `{username, password}`. auth brokers the password grant to Keycloak, resolves the
account by `identity_id` (= `iss#sub`), and mints an ES256 edge token + refresh.

```bash
grpcurl -plaintext -d '{"device":{"user_agent":"cli","ip_address":"127.0.0.1","device_id":"d1"},
  "grant_type":"PASSWORD","password":{"username":"alice","password":"password"}}' \
  localhost:50060 auth.v1.AuthService/Login
```

## Verified working end-to-end

- ✅ Real login (auth → Keycloak password grant → ES256 token); wrong password rejected.
- ✅ Timeline fan-out — alice's following-feed returns bob's + carol's 10 posts.
- ✅ Search — OpenSearch indexed 3 profiles + 15 posts; queries return hits.
- ✅ Accounts (Active), profiles, follows, published posts.

## Known caveats / follow-ups

- **Single-node Scylla → RF=1.** Migrations create keyspaces at RF=3 (prod topology);
  a single node can't satisfy `LOCAL_QUORUM` writes, so `scylla-rf1` downshifts every
  service keyspace to RF=1 after migration. (If you add a Scylla-backed service, add
  its keyspace to that one-shot's list.)
- **Media (images + video).** `media-server` (`:50063`) reaches MinIO in-network
  (`minio:9000`) for byte I/O, but presigns upload/download URLs against
  `localhost:9000` (`MEDIA_OBJECT_STORE_PUBLIC_ENDPOINT`) so a host/browser client can
  resolve them. Flow: `IssueUploadTicket` → PUT the bytes to the presigned URL →
  `CommitUpload` → the asset reaches `READY` (images) or, for video, `media-worker`
  (`:50071`) transcodes it with ffmpeg to a 3-rung HLS ladder + poster and marks it
  `READY`. `ResolveDelivery` then returns the playback URL — for video, an HLS
  `master.m3u8` under `http://localhost:9000/media/post-videos/<hash>/` (MinIO serves
  the bucket with anonymous read, so hls.js can play it directly). Video kinds:
  `MEDIA_KIND_VIDEO`, `video/mp4` + `video/quicktime`, 200 MiB cap.
- **Counters reconcile async.** counter-server/worker are up and respond, but the
  FOLLOWER magnitude reads 0 — follower/following counts are a window onto social-graph's
  set and the reconcile path isn't populating in this stack.
- **realtime notification push.** `realtime-dispatcher` subscribes to `notification.v1.events`,
  which the event-topology registry does **not** provision (no producer). Non-fatal
  (dispatcher stays up, retries) — a genuine registry/consumer inconsistency to resolve
  in code, not here. Other realtime channels + the WSS gateway (`ws://localhost:8443`) are up.
- **`ProfileService/ListProfilesByAccount` is broken** (CQL bug: binds `i64` for a `LIMIT`
  column typed `int`). Use `GetProfileByHandle` / `GetProfileById` instead (the seed does).
- No transport-level JWT enforcement — services accept direct calls without a token
  during dev; auth is exercised via the login flow above.

`docker-compose.backends.yml` remains the infra-only smoke stack.
