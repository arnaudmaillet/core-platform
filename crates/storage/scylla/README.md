# scylla-storage

Production-grade ScyllaDB session management crate for the `core-platform` workspace.

Provides:
- Token-aware, DC-aware session construction with a prepared-statement LRU cache (`CachingSession`)
- Three built-in execution profiles (`Strict`, `Fast`, `Analytical`)
- OTel tracing bridge via `HistoryListener` integration
- Structured error mapping with `AppError`-compatible `SDB-XXXX` codes
- Environment-driven configuration and health check utilities

## Architectural boundaries

This crate is a **pure infrastructure capability** crate. It contains no domain tables, CQL schemas, or application models. All ScyllaDB interaction (keyspace DDL, table DDL, application queries) belongs in the service crates that depend on this one.

## Quick start

```rust
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder, ProfileKind};
use scylla_storage::health::health_check;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ScyllaConfig::from_env();
    let client = ScyllaSessionBuilder::new(config).build().await?;

    health_check(&client.session).await?;
    println!("cluster is healthy");
    Ok(())
}
```

### Attaching the OTel listener per statement

```rust
use std::sync::Arc;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;

let mut stmt = Statement::new("INSERT INTO feed.events (id, ts) VALUES (?, ?)");
stmt.set_history_listener(
    Arc::clone(&client.history_listener) as Arc<dyn HistoryListener>
);
stmt.set_execution_profile_handle(
    client.profiles.get(ProfileKind::Strict).clone().into_handle(),
);
client.session.query_unpaged(stmt, (id, ts)).await?;
```

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `SCYLLA_CONTACT_POINTS` | `127.0.0.1:9042` | Comma-separated `host:port` list |
| `SCYLLA_LOCAL_DC` | `datacenter1` | Preferred datacenter for DC-aware LBP |
| `SCYLLA_KEYSPACE` | _(none)_ | Optional keyspace set on the session |
| `SCYLLA_USERNAME` | _(none)_ | CQL authenticator username |
| `SCYLLA_PASSWORD` | _(none)_ | CQL authenticator password |
| `SCYLLA_COMPRESSION` | `lz4` | Wire compression: `none`, `lz4`, `snappy` |
| `SCYLLA_CONNECT_TIMEOUT_SECS` | `5` | TCP connection timeout in seconds |
| `SCYLLA_REQUEST_TIMEOUT_SECS` | `2` | Default per-request timeout (overridden per profile) |
| `SCYLLA_STATEMENT_CACHE_CAPACITY` | `1000` | Maximum prepared-statement cache entries |

## Execution profiles

| Profile | Consistency | Speculative | Timeout | Use case |
|---|---|---|---|---|
| `Strict` | `LocalQuorum` | no | 5 s | Mutations — feed writes, follow-graph insertions |
| `Fast` | `LocalOne` | +1 attempt / 50 ms delay | 2 s | Latency-sensitive reads — timelines, feed lookups |
| `Analytical` | `Quorum` | no | 30 s | Background aggregation, admin reads |

`Strict` is registered as the session-level default. Statements that do not attach an explicit profile handle use `Strict`.

## Error code table

| Code | Variant | Retryable | Severity |
|---|---|---|---|
| `SDB-1001` | `WriteTimeout` | yes | High |
| `SDB-1002` | `ReadTimeout` | yes | High |
| `SDB-1003` | `Unavailable` | yes | Critical |
| `SDB-1004` | `Overloaded` | yes | High |
| `SDB-1005` | `RateLimitReached` | yes | High |
| `SDB-1006` | `IsBootstrapping` | yes | Medium |
| `SDB-1007` | `ClientTimeout` | yes | High |
| `SDB-2001` | `ConnectionPool` | yes | High |
| `SDB-2002` | `Transport` | yes | High |
| `SDB-3001` | `AuthenticationError` | no | Critical |
| `SDB-3002` | `Unauthorized` | no | Low |
| `SDB-4001` | `AlreadyExists` | no | Low |
| `SDB-5001` | `BadQuery` | no | Critical |
| `SDB-5002` | `QueryInvalid` | no | Medium |
| `SDB-5003` | `WriteFailure` | no | High |
| `SDB-5004` | `ReadFailure` | no | High |
| `SDB-6001` | `SchemaConflict` | no | High |
| `SDB-7001` | `Bootstrap` | no | Critical |
| `SDB-8001` | `Configuration` | no | Critical |
| `SDB-8002` | `ProtocolError` | no | Critical |
| `SDB-9000` | `Unknown` | no | Medium |

## OTel span hierarchy

```
[caller's active span]
  └── scylla.request               ← full query lifecycle
        ├── scylla.attempt         ← primary coordinator round-trip
        └── scylla.speculative_fiber
              └── scylla.attempt   ← speculative backup (Fast profile)
```

Spans carry `otel.kind = CLIENT`, `db.system = scylladb`, `net.peer.name`, and `net.peer.port` per OTel semantic conventions.

## Running integration tests

```sh
# Start a local ScyllaDB node
docker run --rm -p 9042:9042 scylladb/scylla --developer-mode=1

# Run all tests including ignored integration tests
SCYLLA_CONTACT_POINTS=127.0.0.1:9042 \
SCYLLA_LOCAL_DC=datacenter1 \
cargo test -p scylla-storage -- --include-ignored
```

## Module layout

```
src/
├── config/
│   └── cluster.rs       ScyllaConfig + CompressionKind
├── error/
│   └── map.rs           ScyllaStorageError + AppError impl + From<ExecutionError>
├── health/
│   └── check.rs         health_check(session) → system.local probe
├── listener/
│   └── otel.rs          OtelHistoryListener → tracing span bridge
├── profile/
│   ├── builder.rs       ProfileBuilder fluent API
│   └── registry.rs      ProfileRegistry (Strict / Fast / Analytical)
└── session/
    └── builder.rs       ScyllaSessionBuilder → ScyllaClient
```
