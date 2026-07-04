# `scylla` — Token-aware ScyllaDB sessions with execution profiles, OTel tracing, and stable error codes

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `storage` — pure infrastructure capability crate (session lifecycle, no schema) |
> | **Package** | `scylla-storage` (dir: `crates/storage/scylla`) |
> | **Consumed by** | `chat`, `profile`, `social-graph`, `post`, `comment`, `geo-discovery`, `notification`, `timeline` |
> | **Depends on** | `scylla` 1.5 (driver), `telemetry`, `error` |
> | **Stability** | stable contract |
> | **Feature flags** | none crate-specific |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`scylla-storage` is the fleet's ScyllaDB session-management crate. It provides token-aware,
DC-aware session construction backed by a prepared-statement LRU cache (`CachingSession`), three
built-in execution profiles (`Strict` / `Fast` / `Analytical`), an OTel tracing bridge via the
driver's `HistoryListener`, structured error mapping with `AppError`-compatible `SDB-XXXX` codes,
and environment-driven configuration + health checks.

**Architectural boundary** — this is a **pure infrastructure capability** crate: it contains **no
domain tables, no CQL schema, and no application models**. All ScyllaDB interaction (keyspace DDL,
table DDL, application queries) belongs in the service crates that depend on it. Stating that
boundary is the point — it keeps storage policy out of the data layer.

---

## 📐 Architecture & key decisions

```
ScyllaConfig::from_env() ─► ScyllaSessionBuilder::build() ─► ScyllaClient {
    session: CachingSession,            // prepared-statement LRU cache
    profiles: ProfileRegistry,          // Strict | Fast | Analytical handles
    history_listener: Arc<dyn HistoryListener>,   // OTel span bridge
}
```

- **`CachingSession`, not raw `Session`** — the cache prepares-and-reuses statements transparently,
  so callers pass CQL strings without managing a prepared-statement registry. The cache size is
  bounded (`SCYLLA_STATEMENT_CACHE_CAPACITY`).
- **Execution profiles are first-class, registered once** — `Strict` (mutations), `Fast`
  (latency-sensitive reads, with speculative execution), `Analytical` (background/admin). `Strict`
  is the session-level default; a statement opts into another profile by attaching its handle. This
  pushes the read/write consistency tiering decision to the call site, where it belongs.
- **OTel bridge is per-statement, not global** — the driver exposes tracing through a
  `HistoryListener` attached *per statement*, so the span bridge is opt-in on the statements that
  matter rather than wrapping every round-trip.
- **Errors are mapped to stable codes at the boundary** — driver `ExecutionError` variants collapse
  into `SDB-XXXX` codes with fixed retryable/severity classification, so consumers branch on a
  stable contract instead of matching driver internals.

---

## 🔌 Public API & contract

```rust
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder, ScyllaClient, ProfileKind};
use scylla_storage::health::health_check;

pub struct ScyllaConfig { /* … */ }
impl ScyllaConfig { pub fn from_env() -> Self; }

pub struct ScyllaSessionBuilder { /* … */ }
impl ScyllaSessionBuilder {
    pub fn new(config: ScyllaConfig) -> Self;
    pub async fn build(self) -> Result<ScyllaClient, ScyllaStorageError>;
}

pub struct ScyllaClient {
    pub session: CachingSession,
    pub profiles: ProfileRegistry,            // .get(ProfileKind::Strict) -> handle
    pub history_listener: Arc<dyn HistoryListener>,
}

pub enum ProfileKind { Strict, Fast, Analytical }

pub async fn health_check(session: &CachingSession) -> Result<(), ScyllaStorageError>; // system.local probe
```

> **Contract notes:** `Strict` is the session default — statements with no explicit profile handle
> run under it. The crate owns session lifecycle and profiles; it does **not** own or validate
> schema. `health_check` probes `system.local` and is the canonical liveness signal a service wires
> into its `health_probes`.

---

## 📦 Integration

```toml
[dependencies]
scylla-storage = { workspace = true }
```

```rust
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder, ProfileKind};
use scylla_storage::health::health_check;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ScyllaSessionBuilder::new(ScyllaConfig::from_env()).build().await?;
    health_check(&client.session).await?;
    println!("cluster is healthy");
    Ok(())
}
```

### Attaching the OTel listener + a profile per statement

```rust
use std::sync::Arc;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;

let mut stmt = Statement::new("INSERT INTO feed.events (id, ts) VALUES (?, ?)");
stmt.set_history_listener(Arc::clone(&client.history_listener) as Arc<dyn HistoryListener>);
stmt.set_execution_profile_handle(client.profiles.get(ProfileKind::Strict).clone().into_handle());
client.session.query_unpaged(stmt, (id, ts)).await?;
```

---

## ⚙️ Configuration & feature flags

| Variable | Default | Description |
|---|---|---|
| `SCYLLA_CONTACT_POINTS` | `127.0.0.1:9042` | Comma-separated `host:port` list |
| `SCYLLA_LOCAL_DC` | `datacenter1` | Preferred datacenter for DC-aware load balancing |
| `SCYLLA_KEYSPACE` | _(none)_ | Optional keyspace set on the session |
| `SCYLLA_USERNAME` | _(none)_ | CQL authenticator username |
| `SCYLLA_PASSWORD` | _(none)_ | CQL authenticator password |
| `SCYLLA_COMPRESSION` | `lz4` | Wire compression: `none`, `lz4`, `snappy` |
| `SCYLLA_CONNECT_TIMEOUT_SECS` | `5` | TCP connection timeout in seconds |
| `SCYLLA_REQUEST_TIMEOUT_SECS` | `2` | Default per-request timeout (overridden per profile) |
| `SCYLLA_STATEMENT_CACHE_CAPACITY` | `1000` | Maximum prepared-statement cache entries |

**Execution profiles:**

| Profile | Consistency | Speculative | Timeout | Use case |
|---|---|---|---|---|
| `Strict` | `LocalQuorum` | no | 5 s | Mutations — feed writes, follow-graph insertions |
| `Fast` | `LocalOne` | +1 attempt / 50 ms delay | 2 s | Latency-sensitive reads — timelines, feed lookups |
| `Analytical` | `Quorum` | no | 30 s | Background aggregation, admin reads |

**Feature flags:** none crate-specific.

---

## 🧯 Error model

`ScyllaStorageError` implements `error::AppError` (via `From<ExecutionError>`); codes map to gRPC
`Status` / HTTP through the shared `error` crate.

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

---

## 🔭 Observability

```
[caller's active span]
  └── scylla.request               ← full query lifecycle
        ├── scylla.attempt         ← primary coordinator round-trip
        └── scylla.speculative_fiber
              └── scylla.attempt   ← speculative backup (Fast profile)
```

Spans carry `otel.kind = CLIENT`, `db.system = scylladb`, `net.peer.name`, and `net.peer.port` per
OTel semantic conventions.

---

## 🧪 Testing

```bash
cargo test   -p scylla-storage
cargo clippy -p scylla-storage --all-targets
```

Integration tests require a live node (they are `#[ignore]` by default):

```bash
docker run --rm -p 9042:9042 scylladb/scylla --developer-mode=1
SCYLLA_CONTACT_POINTS=127.0.0.1:9042 SCYLLA_LOCAL_DC=datacenter1 \
  cargo test -p scylla-storage -- --include-ignored
```

---

## 🗂️ Module layout

```
src/
├── config/cluster.rs       ScyllaConfig + CompressionKind
├── error/map.rs            ScyllaStorageError + AppError impl + From<ExecutionError>
├── health/check.rs         health_check(session) → system.local probe
├── listener/otel.rs        OtelHistoryListener → tracing span bridge
├── profile/
│   ├── builder.rs          ProfileBuilder fluent API
│   └── registry.rs         ProfileRegistry (Strict / Fast / Analytical)
└── session/builder.rs      ScyllaSessionBuilder → ScyllaClient
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. Package name is `scylla-storage`, but the upstream driver crate is `scylla`.**
The dependency `-p` flag and `Cargo.toml` key are `scylla-storage`; `use scylla::…` refers to the
**driver**, `use scylla_storage::…` to this crate. Mixing them up is the most common build error.

**2. A statement ran under the wrong consistency.**
A statement with **no** attached profile handle runs under the session default (`Strict` /
`LocalQuorum`). Latency-sensitive reads must explicitly attach the `Fast` handle —
`set_execution_profile_handle(client.profiles.get(ProfileKind::Fast)…)` — or they silently pay
quorum latency.

**3. No spans appear for my queries.**
The OTel `HistoryListener` is attached **per statement**, not globally. A statement without
`set_history_listener(...)` emits no `scylla.request` span. Attach the listener on the statements
you want traced.

**4. `scylla` 1.5 API drift.**
This crate targets the `scylla` 1.5 driver; the `CachingSession` / `Statement` / `HistoryListener`
APIs shifted across 1.x. Pin and bump deliberately — a minor driver bump can move these surfaces.
