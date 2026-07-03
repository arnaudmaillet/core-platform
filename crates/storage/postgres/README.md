# `postgres` — Topology-aware PostgreSQL storage with deterministic shard routing

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `storage` — connection pool + shard router + transaction manager (no schema) |
> | **Package** | `postgres` (dir: `crates/storage/postgres`) |
> | **Consumed by** | `account` (and any future Postgres-backed service) |
> | **Depends on** | `sqlx`, `seahash`, `tokio`, `telemetry`, `error`, `health` |
> | **Stability** | stable contract (`run_on_shard` API frozen) |
> | **Feature flags** | none |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`postgres` is the authoritative PostgreSQL infrastructure layer: a production-grade connection pool, a
deterministic shard router, and a topology-aware transaction manager behind one ergonomic API. Code
written against `run_on_shard()` compiles and behaves identically under both `SingleNode`
(CockroachDB / Aurora) and `ApplicationSharded` (manual sharding) topologies — **zero call-site
changes**.

**Architectural boundary** — it owns connection lifecycle, routing, and transactions; it owns **no
schema, no migrations, no domain models**. Cross-shard atomicity is intentionally out of scope:
`run_on_shard` is ACID *within a single shard*; cross-shard consistency must use the outbox pattern or
sagas.

---

## 📐 Architecture & key decisions

```
SingleNode (PG_TOPOLOGY=single)            ApplicationSharded (PG_TOPOLOGY=sharded)
TransactionManager(Arc<Topology>)          TransactionManager(Arc<Topology>)
   run() / run_on_shard()                     run_on_shard(key, f)
        ▼                                          ▼ deterministic_shard_id(key, n) = SeaHash % shard_count
   PgPool (global, engine-routed)             ShardCluster → PgPool[0..N]
   CockroachDB / Aurora                       shard-0 … shard-N
```

- **Feed-based `ShardKey`, not `&[u8]`** — a `fn shard_bytes(&self) -> &[u8]` signature can't return a
  reference to stack bytes (`u64::to_le_bytes()` drops at method end). Mirroring `std::hash::Hash`
  (implementors push bytes into a generic `Hasher`) is zero-allocation for every type.
- **Determinism + immutability** — `(key bytes, shard_count)` always maps to the same `ShardId` across
  restarts/machines; `ShardCluster` is built once behind `Arc`, no locks on the read path.
- **Fail-fast cluster build** — `PgClusterBuilder` refuses to start if any shard pool fails to connect;
  a partial cluster is never allowed at runtime.
- **Retry policy lives upstream** — `Deadlock`/`SerializationFailure`/`PoolTimedOut` are flagged
  retryable but **not** retried here; that's the `resilience` layer / CQRS middleware's job.

---

## 🔌 Public API & contract

```rust
pub struct TransactionManager { /* Arc<Topology> */ }
pub type PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;
pub enum TopologyConfig { SingleNode(PostgresConfig), ApplicationSharded(ShardedPostgresConfig) }

pub trait ShardKey { fn hash_shard_key<H: std::hash::Hasher>(&self, state: &mut H); }  // mirrors std::hash::Hash

impl TransactionManager {
    pub fn new(pool: PgPool) -> Self;                  // SingleNode
    pub fn from_cluster(cluster: ShardCluster) -> Self;// ApplicationSharded
    pub fn pool(&self) -> &PgPool;                     // panics in sharded mode
    pub fn pool_for<K: ShardKey + ?Sized>(&self, key: &K) -> Result<&PgPool, StorageError>;

    /// SingleNode only — ShardRoutingFailed in sharded mode. Backward-compatible with old call sites.
    pub async fn run<F, T, E>(&self, f: F) -> Result<T, E> where /* F: FnOnce(&mut PgTransaction) -> BoxFuture<…> */;
    /// Topology-agnostic — PREFERRED. Key ignored in SingleNode; routes deterministically when sharded.
    pub async fn run_on_shard<K: ShardKey + ?Sized, F, T, E>(&self, key: &K, f: F) -> Result<T, E>;
}
```

`ShardKey` has blanket impls for `Uuid`, `String`/`str`, `u64`/`u128`/`i64`, `[u8; N]`/`[u8]` (all
zero-allocation; DST types need the `?Sized` bound).

> **Contract notes:** new service code should use `run_on_shard` exclusively — it's topology-agnostic.
> `TransactionManager::clone()` is O(1) (one `Arc` bump). sqlx's `Transaction` `Drop` issues a
> best-effort rollback if the executor is cancelled mid-transaction.

---

## 🧯 Error model

`StorageError` (15 variants) implements `error::AppError`:

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

## 📦 Integration

```toml
[dependencies]
postgres = { workspace = true }
```

```rust
use postgres::{TopologyBuilder, TopologyConfig, TransactionManager};

let tx_manager: TransactionManager = TopologyBuilder::build(TopologyConfig::from_env()).await?;

// Topology-agnostic — unchanged whether SingleNode or ApplicationSharded.
tx_manager.run_on_shard(&account_id, |tx| Box::pin(async move {
    sqlx::query("INSERT INTO accounts (id) VALUES ($1)").bind(account_id)
        .execute(&mut **tx).await.map_err(MyError::from)
})).await?;
```

`health_check(pool)` / `health_check_cluster(cluster)` (probes all shards concurrently) back the
service's readiness.

---

## ⚙️ Configuration & feature flags

**SingleNode** (`PG_TOPOLOGY=single` or unset):

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | **Yes** | — | libpq connection string |
| `PG_MAX_CONNECTIONS` / `PG_MIN_CONNECTIONS` | No | `20` / `2` | Pool ceiling / warm idle |
| `PG_ACQUIRE_TIMEOUT_SECS` | No | `5` | Max wait for a free slot → `PoolTimedOut` |
| `PG_IDLE_TIMEOUT_SECS` / `PG_MAX_LIFETIME_SECS` | No | `600` / `1800` | Idle reaping / max age |
| `PG_SLOW_STATEMENT_THRESHOLD_MS` | No | `1000` | WARN threshold for slow queries |

**ApplicationSharded** (`PG_TOPOLOGY=sharded`): all of the above per shard, plus `PG_SHARD_COUNT`
(**immutable after first deploy** — changing it remaps every key) and `PG_SHARD_<N>_URL` for each `N`
in `[0, PG_SHARD_COUNT)`.

One cargo feature: `integration-postgres` gates the live test suites (see Testing below).

---

## 🔭 Observability

OTel spans: `postgres.pool.build`, `postgres.cluster.build` (`shard_count`), `postgres.topology.build`,
`db.transaction`, `db.transaction.sharded` (`shard_id`), `postgres.health_check[_cluster]`.

Suggested alerts: `DB-3001` rate > 0 for 30s ⇒ page; `DB-2001` > 5/min ⇒ warn; any `DB-8001` ⇒ page
(misconfig); p99 query latency > slow threshold ⇒ warn.

---

## 🧪 Testing

```bash
cargo test   -p postgres                  # routing/hashing/error-mapping/ShardKey — no DB
cargo clippy -p postgres --all-targets
# Live suites (constraint violations, transaction rollback) are opt-in via the
# fleet's `integration-<crate>` convention and need a real Postgres:
docker compose up -d postgres
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test -p postgres --features integration-postgres
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `DB-8002 ShardRoutingFailed` at runtime.**
`run()` (unkeyed) was called while `PG_TOPOLOGY=sharded` — it can't pick a shard. Replace with
`run_on_shard(&shard_key, …)`; any value uniquely identifying the owning entity (`AccountId`) qualifies.

**2. `DB-8001 ShardNotFound` at startup / first request.**
`PG_SHARD_COUNT` doesn't match the set of `PG_SHARD_<N>_URL` vars, or one URL is missing. Every integer
in `[0, PG_SHARD_COUNT)` needs a URL — fix the env and restart.

**3. `DB-3001 PoolTimedOut` under load.**
`PG_MAX_CONNECTIONS` too low, or a long transaction holds connections. Raise the ceiling; in sharded
mode inspect per-shard `db.transaction.sharded` span durations for a hot shard (key skew) exhausting
its pool while others idle.

**4. Changing `PG_SHARD_COUNT` "rebalanced" everything wrong.**
It's **immutable** after first deploy — changing it remaps every key to a different shard. A shard-count
change is a full data-resharding operation, not a config tweak.
