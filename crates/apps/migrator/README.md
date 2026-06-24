# migrator

The fleet schema-migration runner. Applies each service's migrations against the
right backend — **ScyllaDB** for the `.cql` services, **PostgreSQL** for
`account`'s `.sql` — tracked and idempotent.

- **Self-contained.** Migrations are embedded at compile time (`include_dir!`), so
  the binary needs no source tree or mounted volume in the container.
- **Tracked.** Each applied `(service, version)` is recorded — in
  `migrations.applied` (Scylla) and `schema_migrations` (Postgres) — so re-running
  skips what's already done. Files use `IF NOT EXISTS`, so a partial run is safe to
  resume.
- **Ordered.** Files apply in filename order (the numeric prefix). The Scylla
  session connects with no keyspace; the keyspace is created by each service's
  `0001` migration and every statement is keyspace-qualified.

## Usage

```bash
migrator            # migrate every service
migrator chat       # migrate one service
```

Connection settings come from the same env vars the services read — `SCYLLA_*`,
`REDIS_*` (unused here), `KAFKA_*` (unused here), and `DATABASE_URL` / `PG_*` for
Postgres.

## End-to-end smoke test (local)

Validates the whole runtime path — migrate → boot a `*-server` → gRPC health —
against real backends.

```bash
# 1. Start the local backend stack (Scylla, Redis, Redpanda/Kafka, Postgres).
docker compose -f local-dev/docker-compose.backends.yml up -d
# wait until all are healthy:
docker compose -f local-dev/docker-compose.backends.yml ps

# 2. Provision schema (defaults match the compose stack).
export SCYLLA_CONTACT_POINTS=127.0.0.1:9042
export DATABASE_URL=postgres://core:core@127.0.0.1:5432/core_platform
cargo run -p migrator

# 3. Boot a service against the same backends + a real infrastructure.toml.
export INFRA_CONFIG_PATH=crates/shared/infra-config/examples/infrastructure.toml
export REDIS_HOSTS=127.0.0.1:6379
export KAFKA_BROKERS=127.0.0.1:9092
export SCYLLA_KEYSPACE=chat
cargo run -p chat-server          # listens on 0.0.0.0:50051

# 4. In another shell, confirm readiness flips to SERVING once probes pass.
grpcurl -plaintext localhost:50051 grpc.health.v1.Health/Check
# → { "status": "SERVING" }
grpcurl -plaintext localhost:50051 list      # reflection lists chat.v1.ChatService
```

> Requires Docker and `grpcurl`. `account-server` uses Postgres (`DATABASE_URL`);
> the Scylla/Redis services additionally need their `SCYLLA_KEYSPACE` set to the
> service keyspace.
