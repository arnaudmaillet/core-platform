# postgres — Topology-aware PostgreSQL storage layer with deterministic shard routing

## 🎯 Overview & Service Role

This crate is the **authoritative PostgreSQL infrastructure layer** for the core-platform workspace. It provides a production-grade connection pool, a deterministic shard router, and a topology-aware transaction manager that abstracts over two execution modes — all behind a single, ergonomic API.

**Critical problems solved:**

- **Topology lock-in** — Services written against `run_on_shard()` compile and behave correctly under both `SingleNode` (CockroachDB / Aurora) and `ApplicationSharded` (manual sharding) topologies with **zero code changes** at the call site.
- **Shard routing complexity** — The crate owns all routing logic: SeaHash-based deterministic key mapping, shard registry management, and per-shard pool lifecycle. Upstream services provide a shard key; the crate does everything else.
- **Observability at the infrastructure layer** — Every transaction open, commit, rollback, slow statement, and shard resolution is automatically instrumented via the platform's OTel-wired `tracing` subscriber.

---

## 📐 Architecture & Concepts

### Topology modes

```
PG_TOPOLOGY=single (default)            PG_TOPOLOGY=sharded
──────────────────────────────          ──────────────────────────────────────

  ┌─────────────────────────┐             ┌─────────────────────────────────┐
  │   TransactionManager    │             │      TransactionManager          │
  │   (Arc<Topology>)       │             │      (Arc<Topology>)             │
  └──────────┬──────────────┘             └────────────┬────────────────────┘
             │                                         │
             │  run() / run_on_shard()                 │  run_on_shard(key, f)
             ▼                                         ▼
  ┌─────────────────────────┐       ┌──────────────────────────────────────┐
  │       PgPool            │       │           ShardCluster               │
  │  (global, engine-routed)│       │   deterministic_shard_id(key, n)     │
  └─────────────────────────┘       │   SeaHash → shard_id % shard_count   │
             │                      └────┬───────────┬────────────┬────────┘
             │                           │           │            │
             ▼                           ▼           ▼            ▼
     CockroachDB / Aurora          PgPool[0]   PgPool[1]   PgPool[N]
     (self-routing cluster)        shard-0     shard-1     shard-N
```

### Key invariants

- **Determinism**: given the same `(key bytes, shard_count)`, `deterministic_shard_id` always returns the same `ShardId` across restarts, deployments, and machines.
- **Immutability after startup**: `ShardCluster` is constructed once and held behind `Arc`; no locks are taken on the read path.
- **Cross-shard atomicity is intentionally out of scope**: `run_on_shard` provides ACID within a single shard. Cross-shard consistency must use the outbox pattern or distributed sagas.

### Resilience guarantees & high-load behaviour

| Concern | Behaviour |
|---|---|
| **Connection exhaustion** | Pool blocks up to `PG_ACQUIRE_TIMEOUT_SECS`; returns `StorageError::PoolTimedOut` (retryable). The `resilience` crate's retry layer should wrap callers. |
| **Deadlock / serialisation failure** | Mapped to `StorageError::Deadlock` / `SerializationFailure` (both `is_retryable = true`). No automatic retry here — retry policy belongs in CQRS middleware. |
| **Shard pool failure at startup** | `PgClusterBuilder` fails fast if any shard pool fails to connect. A partial cluster is never allowed at runtime. |
| **Shard health degradation** | `health_check_cluster` returns a per-shard result map; the caller decides the liveness policy (e.g., tolerate ≤ 1 degraded shard). |
| **Idle connection reaping** | Governed by `PG_IDLE_TIMEOUT_SECS` (default 600 s) and `PG_MAX_LIFETIME_SECS` (default 1800 s). Prevents stale connection accumulation under low traffic. |
| **Rollback on panic** | sqlx's `Drop` impl on `Transaction` issues a best-effort rollback, providing a safety net if the executor is cancelled mid-transaction. |

---

## 🔌 Public Interfaces & API Contract

### Core types

```rust
// Backward-compatible (unchanged from v1)
pub struct PgPoolBuilder;
pub struct TransactionManager { topology: Arc<Topology> }
pub type  PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;
pub enum  StorageError { /* 15 variants, DB-1001 … DB-9000 */ }

// Topology-aware additions
pub struct TopologyBuilder;
pub struct PgClusterBuilder;
pub struct ShardCluster    { shards: HashMap<ShardId, PgPool>, shard_count: u16 }
pub struct ShardId(pub u16);

pub enum TopologyConfig {
    SingleNode(PostgresConfig),
    ApplicationSharded(ShardedPostgresConfig),
}

pub trait ShardKey {
    // Feed-based pattern — mirrors std::hash::Hash; zero-allocation for all types.
    fn hash_shard_key<H: std::hash::Hasher>(&self, state: &mut H);
}
```

### `TransactionManager` — primary API

```rust
impl TransactionManager {
    // ── Construction ──────────────────────────────────────────────────────────
    pub fn new(pool: PgPool) -> Self;                          // SingleNode
    pub fn from_cluster(cluster: ShardCluster) -> Self;       // ApplicationSharded

    // ── Pool access ───────────────────────────────────────────────────────────
    pub fn pool(&self) -> &PgPool;                            // panics in sharded mode
    pub fn pool_for<K: ShardKey + ?Sized>(&self, key: &K) -> Result<&PgPool, StorageError>;

    // ── Transactions ──────────────────────────────────────────────────────────

    /// SingleNode only. Returns ShardRoutingFailed in ApplicationSharded mode.
    /// Backward-compatible with all existing call sites.
    pub async fn run<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'tx> FnOnce(&'tx mut PgTransaction) -> BoxFuture<'tx, Result<T, E>>,
        E: From<StorageError> + Send,
        T: Send;

    /// Topology-agnostic. Preferred entry point for all new service code.
    /// Key is ignored in SingleNode mode; routes deterministically in sharded mode.
    pub async fn run_on_shard<K, F, T, E>(&self, key: &K, f: F) -> Result<T, E>
    where
        K: ShardKey + ?Sized,
        F: for<'tx> FnOnce(&'tx mut PgTransaction) -> BoxFuture<'tx, Result<T, E>>,
        E: From<StorageError> + Send,
        T: Send;
}
```

### `ShardKey` — why the feed pattern, not `&[u8]`

The naive signature `fn shard_bytes(&self) -> &[u8]` fails for stack-allocated types: `u64::to_le_bytes()` produces a `[u8; 8]` that lives on the stack and is dropped at the end of the method — no valid lifetime for a returned reference exists without a heap allocation.

This crate mirrors `std::hash::Hash`: implementors push bytes into a generic `Hasher` via `write_*` methods. The routing layer supplies the hasher, calls `finish()`, and applies the modulo — entirely zero-allocation for every type.

```rust
impl ShardKey for AccountId {
    fn hash_shard_key<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash_shard_key(state); // delegate to the Uuid impl
    }
}
```

### `ShardKey` blanket implementations

| Type | Feed method | Notes |
|---|---|---|
| `uuid::Uuid` | `write(as_bytes())` | 16 bytes, no allocation |
| `String` | `write(as_bytes())` | borrowed slice |
| `str` | `write(as_bytes())` | DST — `?Sized` bound required |
| `u64` | `write_u64(v)` | 8 bytes, zero allocation |
| `u128` | `write_u128(v)` | 16 bytes, zero allocation |
| `i64` | `write_i64(v)` | 8 bytes, zero allocation |
| `[u8; N]` | `write(as_slice())` | const generics, no allocation |
| `[u8]` | `write(self)` | DST — `?Sized` bound required |

### `StorageError` codes

| Code | Variant | Severity | Retryable |
|---|---|---|---|
| DB-1001 | `UniqueViolation` | Low | No |
| DB-1002 | `ForeignKeyViolation` | Medium | No |
| DB-1003 | `NotNullViolation` | Medium | No |
| DB-1004 | `CheckViolation` | Low | No |
| DB-2001 | `Deadlock` | High | **Yes** |
| DB-2002 | `SerializationFailure` | High | **Yes** |
| DB-3001 | `PoolTimedOut` | High | **Yes** |
| DB-3002 | `PoolClosed` | Critical | No |
| DB-4001 | `RowNotFound` | Low | No |
| DB-5001 | `Migration` | Critical | No |
| DB-6001 | `Connection` | High | No |
| DB-7001 | `Configuration` | Critical | No |
| DB-8001 | `ShardNotFound` | Critical | No |
| DB-8002 | `ShardRoutingFailed` | Critical | No |
| DB-9000 | `Database` | Medium | No |

---

## 📦 Integration & Usage

### Cargo.toml

```toml
[dependencies]
postgres-storage = { workspace = true }
```

### Single-node bootstrap (CockroachDB / Aurora)

```rust
use postgres::{TopologyBuilder, TopologyConfig, TransactionManager};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // PG_TOPOLOGY is unset or "single" → SingleNode mode.
    let config = TopologyConfig::from_env();
    let tx_manager: TransactionManager = TopologyBuilder::build(config).await?;

    // Topology-agnostic call — works unchanged if topology later switches to sharded.
    tx_manager
        .run_on_shard(&account_id, |tx| Box::pin(async move {
            sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
                .bind(account_id)
                .execute(&mut **tx)
                .await
                .map_err(MyError::from)?;
            Ok(())
        }))
        .await?;

    Ok(())
}
```

### Application-sharded bootstrap

```bash
# Environment
PG_TOPOLOGY=sharded
PG_SHARD_COUNT=4
PG_SHARD_0_URL=postgres://user:pass@pg-shard-0/db
PG_SHARD_1_URL=postgres://user:pass@pg-shard-1/db
PG_SHARD_2_URL=postgres://user:pass@pg-shard-2/db
PG_SHARD_3_URL=postgres://user:pass@pg-shard-3/db
```

```rust
// Identical application code — topology resolved at runtime.
let config = TopologyConfig::from_env();  // → ApplicationSharded
let tx_manager = TopologyBuilder::build(config).await?;

tx_manager
    .run_on_shard(&account_id, |tx| Box::pin(async move {
        // Executes on the shard deterministically owning account_id.
        sqlx::query("INSERT INTO accounts (id) VALUES ($1)")
            .bind(account_id)
            .execute(&mut **tx)
            .await
            .map_err(MyError::from)
    }))
    .await?;
```

### Health check integration

```rust
use postgres::{health_check, health_check_cluster};

// SingleNode
async fn readiness(pool: &sqlx::PgPool) -> bool {
    health_check(pool).await.is_ok()
}

// ApplicationSharded — check all shards, fail if any is down
async fn readiness_cluster(cluster: &ShardCluster) -> bool {
    health_check_cluster(cluster)
        .await
        .values()
        .all(|r| r.is_ok())
}
```

---

## ⚙️ Configuration & Runtime Environment

### SingleNode mode (`PG_TOPOLOGY=single` or unset)

| Variable | Required | Default | Description |
|---|---|---|---|
| `PG_TOPOLOGY` | No | `single` | Topology mode: `single` or `sharded` |
| `DATABASE_URL` | **Yes** | — | libpq-compatible connection string |
| `PG_MAX_CONNECTIONS` | No | `20` | Hard ceiling on pool size |
| `PG_MIN_CONNECTIONS` | No | `2` | Warm connections kept alive when idle |
| `PG_ACQUIRE_TIMEOUT_SECS` | No | `5` | Max wait for a free connection slot |
| `PG_IDLE_TIMEOUT_SECS` | No | `600` | Idle connection reaping window (s) |
| `PG_MAX_LIFETIME_SECS` | No | `1800` | Maximum connection age (s) |
| `PG_SLOW_STATEMENT_THRESHOLD_MS` | No | `1000` | WARN threshold for slow queries (ms) |

### ApplicationSharded mode (`PG_TOPOLOGY=sharded`)

All pool tuning variables above apply uniformly across all shard pools, plus:

| Variable | Required | Default | Description |
|---|---|---|---|
| `PG_TOPOLOGY` | **Yes** | — | Must be `sharded` |
| `PG_SHARD_COUNT` | **Yes** | — | Number of shards; must be > 0; **immutable after first deploy** |
| `PG_SHARD_0_URL` | **Yes** | — | Connection string for shard 0 |
| `PG_SHARD_N_URL` | **Yes** | — | Connection string for shard N (0-indexed, up to `PG_SHARD_COUNT - 1`) |

> **Warning**: `PG_SHARD_COUNT` is **immutable** once the cluster is first deployed. Changing it remaps every key to a different shard. A shard count migration requires a full data resharding operation.

---

## 📈 Telemetry, Performance & Metrics

### OTel spans emitted

| Span name | Emitted by | Key fields |
|---|---|---|
| `postgres.pool.build` | `PgPoolBuilder::build` | `db.max_connections`, `db.min_connections` |
| `postgres.cluster.build` | `PgClusterBuilder::build` | `shard_count` |
| `postgres.topology.build` | `TopologyBuilder::build` | — |
| `db.transaction` | `TransactionManager::run` | — |
| `db.transaction.sharded` | `TransactionManager::run_on_shard` | `shard_id` |
| `postgres.health_check` | `health_check` | — |
| `postgres.health_check_cluster` | `health_check_cluster` | `shard_count` |

### Performance characteristics

- `TransactionManager::clone()` — O(1); increments one `Arc` refcount, never copies pool data.
- `run_on_shard()` in SingleNode mode — zero routing overhead; identical path to `run()`.
- `run_on_shard()` in ApplicationSharded mode — one SeaHash pass (< 10 ns on x86-64) + one `HashMap` lookup.
- `health_check_cluster()` — all shards probed **concurrently** via `futures::join_all`.

### Recommended Prometheus alerts

| Alert | Condition | Severity |
|---|---|---|
| `postgres_pool_timed_out` | `DB-3001` error rate > 0 for 30 s | Page |
| `postgres_deadlock_rate` | `DB-2001` rate > 5 / min | Warn |
| `postgres_shard_not_found` | Any `DB-8001` occurrence | Page (misconfiguration) |
| `postgres_slow_statements` | p99 query latency > slow threshold | Warn |

---

## 🛠️ Local Development & Contribution

### Prerequisites

```bash
# Start a local PostgreSQL instance
docker compose up -d postgres

# Export the test database URL
export DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres
```

### Build & lint

```bash
cargo check  -p postgres
cargo clippy -p postgres -- -D warnings
cargo fmt    -p postgres
```

### Test

```bash
# Unit tests only — no database required (routing, hashing, error mapping, shard key impls)
cargo test -p postgres --lib

# Full suite including integration tests
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test -p postgres
```

---

## 🚨 Troubleshooting & Runbook

**`DB-8002 ShardRoutingFailed` at runtime**

- **Root cause**: `TransactionManager::run()` was called while `PG_TOPOLOGY=sharded`. The unkeyed API cannot determine which shard to route to.
- **Fix**: Replace `tx_manager.run(|tx| ...)` with `tx_manager.run_on_shard(&shard_key, |tx| ...)`. Any value that uniquely identifies the entity owning the data qualifies as the key (e.g., `AccountId`, `UserId`).

---

**`DB-8001 ShardNotFound` at startup or on first request**

- **Root cause**: `PG_SHARD_COUNT` does not match the number of `PG_SHARD_<N>_URL` variables, or a shard URL was omitted. The cluster registry has a gap.
- **Fix**: Verify that every integer in `[0, PG_SHARD_COUNT)` has a corresponding `PG_SHARD_<N>_URL`. Re-check the deployment environment config and restart the service.

---

**`DB-3001 PoolTimedOut` under load**

- **Root cause**: `PG_MAX_CONNECTIONS` is too low for the current throughput, or a long-running transaction is holding connections open.
- **Immediate mitigation**: Increase `PG_MAX_CONNECTIONS`. In ApplicationSharded mode, inspect per-shard `db.transaction.sharded` span durations to identify hot shards exhausting their individual pools while others are idle — this indicates a shard key skew problem in the domain data.
