# `postgres` — Production-Grade PostgreSQL Infrastructure for Hyperscale CQRS

## 🎯 Overview & Service Role

`postgres` is the **single source of truth for relational persistence** in the core-platform workspace. It owns the PostgreSQL connection pool lifecycle, maps every `sqlx::Error` to the platform's shared `AppError` contract, and exposes one clean entry point for ACID-compliant multi-model writes: [`TransactionManager::run`].

**Critical problem it solves:** upstream CQRS command handlers must atomically mutate multiple domain models (e.g., `accounts` + `ledger`) without coupling themselves to connection management, error categorisation, or retry classification. This crate provides that abstraction as a domain-agnostic capability layer.

**What this crate deliberately excludes:**
- Domain tables or entity types
- Outbox, idempotency, or optimistic versioning patterns
- Schema migrations (use `sqlx-migrate` directly in the service binary)

**Consumed by:** every CQRS command handler in the workspace that requires ACID guarantees over PostgreSQL.

---

## 📐 Architecture & Concepts

### Data Flow

```
CQRS Command Handler
│
│  let result = tx_mgr.run(|tx| Box::pin(async move {
│      sqlx::query("INSERT INTO accounts …").execute(&mut **tx).await?;
│      sqlx::query("INSERT INTO ledger  …").execute(&mut **tx).await?;
│      Ok(account_id)
│  })).await?;
│
▼
TransactionManager::run()                         (db.transaction span)
│
├── pool.begin() ─────────────────────────────────► PgPool
│                                                     ├── max_connections
│                                                     ├── min_connections
│                                                     ├── acquire_timeout
│                                                     ├── idle_timeout
│                                                     └── max_lifetime
│
├── f(&mut tx)                                    (user closure executes SQL)
│     │
│     └── sqlx ConnectOptions tracing hooks ──────► active tracing subscriber
│           ├── log_statements(level)               (every SQL statement)
│           └── log_slow_statements(WARN, 1 s)      (slow query alert)
│
├── Ok(T)  ──► tx.commit()  ──► Result<T, E>
│
└── Err(E) ──► tx.rollback()
                │
                └── From<sqlx::Error> ─────────────► StorageError
                      ├── ErrorKind::UniqueViolation    → DB-1001  Low
                      ├── ErrorKind::ForeignKeyViolation → DB-1002  Medium
                      ├── ErrorKind::NotNullViolation   → DB-1003  Medium
                      ├── ErrorKind::CheckViolation     → DB-1004  Low
                      ├── SQLSTATE 40P01 (deadlock)     → DB-2001  High  ♻
                      ├── SQLSTATE 40001 (serial fail)  → DB-2002  High  ♻
                      ├── PoolTimedOut                  → DB-3001  High  ♻
                      ├── PoolClosed                    → DB-3002  Critical
                      ├── RowNotFound                   → DB-4001  Low
                      ├── Migrate                       → DB-5001  Critical
                      ├── Io / Tls / Protocol           → DB-6001  High
                      ├── Configuration                 → DB-7001  Critical
                      └── Database (other SQLSTATE)     → DB-9000  Medium

♻ = is_retryable() → true
```

### Resilience Guarantees & High-Load Behaviour

| Concern | Mechanism | Behaviour |
|---|---|---|
| **Connection saturation** | `acquire_timeout` (default 5 s) | Returns `DB-3001 PoolTimedOut` (retryable) instead of blocking indefinitely |
| **Connection leaks** | `idle_timeout` + `max_lifetime` | Idle connections are reaped; old connections are recycled to prevent backend accumulation |
| **Deadlocks** | `From<sqlx::Error>` classification | Deadlocks surface as `DB-2001` with `is_retryable = true`; upstream resilience layer handles back-off |
| **Serialization failures** | Same as deadlocks | `DB-2002`, retryable, identical retry contract |
| **Partial writes** | `tx.rollback()` on `Err(E)` + `Transaction::drop()` safety net | No phantom inserts persist on handler error |
| **Slow queries** | `log_slow_statements(WARN, threshold)` | Automatically logged through the active tracing subscriber; no application change required |
| **Pool monitoring** | `PgPool::size()`, `PgPool::num_idle()` | Expose as Prometheus gauges in the service binary |

---

## 🔌 Public Interfaces & API Contract

### `StorageError` — platform-native error type

```rust
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StorageError {
    UniqueViolation     { constraint: String },   // DB-1001  Low
    ForeignKeyViolation { constraint: String },   // DB-1002  Medium
    NotNullViolation    { detail: String },        // DB-1003  Medium
    CheckViolation      { constraint: String },   // DB-1004  Low
    Deadlock,                                      // DB-2001  High  retryable
    SerializationFailure,                          // DB-2002  High  retryable
    PoolTimedOut,                                  // DB-3001  High  retryable
    PoolClosed,                                    // DB-3002  Critical
    RowNotFound,                                   // DB-4001  Low
    Migration           { message: String },       // DB-5001  Critical
    Connection          { message: String },       // DB-6001  High
    Configuration       { message: String },       // DB-7001  Critical
    Database            { code: String, message: String }, // DB-9000 Medium
}

impl AppError for StorageError { … }
impl From<sqlx::Error> for StorageError { … }
```

**Stability contract:** `error_code` strings (`"DB-1001"` … `"DB-9000"`) are part of the public API surface. Dashboards and alerting rules key off them. Treat any rename as a breaking change.

---

### `TransactionManager` — ACID boundary helper

```rust
#[derive(Clone, Debug)]
pub struct TransactionManager { … }

impl TransactionManager {
    /// O(1) clone — shares the underlying PgPool (Arc<PgPoolInner>).
    pub fn new(pool: PgPool) -> Self;

    /// Raw pool access for non-transactional reads or COPY commands.
    pub fn pool(&self) -> &PgPool;

    /// Execute `f` inside a single ACID transaction.
    ///
    /// # Type constraints
    ///
    /// `E: From<StorageError>` — lets pool-acquire and commit failures
    /// propagate through the handler's own error type with a simple `?`.
    ///
    /// # Commit / rollback contract
    ///
    /// - `f` returns `Ok(T)`  → commit, return `Ok(T)`
    /// - `f` returns `Err(E)` → rollback (best-effort), return `Err(E)`
    /// - `commit()` fails     → return `Err(E::from(StorageError::Connection { … }))`
    pub async fn run<F, T, E>(&self, f: F) -> Result<T, E>
    where
        F: for<'tx> FnOnce(&'tx mut PgTransaction) -> BoxFuture<'tx, Result<T, E>>,
        E: From<StorageError> + Send,
        T: Send;
}

/// Owned PostgreSQL transaction scoped to pool lifetime.
pub type PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;
```

---

### `PgPoolBuilder` — pool construction with tracing hooks

```rust
impl PgPoolBuilder {
    /// Parse `config.database_url`, apply pool tuning, wire statement
    /// logging onto the active `tracing` subscriber, and open
    /// `min_connections` eagerly.
    pub async fn build(config: PostgresConfig) -> Result<PgPool, StorageError>;
}
```

**Tracing hook contract:** sqlx emits one `log` event per statement at `config.statement_log_level` (default `DEBUG`) and one `WARN` event for any statement exceeding `slow_statement_threshold`. These events are automatically captured by the `tracing_subscriber` installed by the `telemetry` crate, inheriting the active distributed trace context.

---

### `PostgresConfig` — typed pool configuration

```rust
pub struct PostgresConfig {
    pub database_url:            String,
    pub max_connections:         u32,
    pub min_connections:         u32,
    pub acquire_timeout:         Duration,
    pub idle_timeout:            Option<Duration>,
    pub max_lifetime:            Option<Duration>,
    pub statement_log_level:     StatementLogLevel,
    pub slow_statement_threshold: Duration,
}

impl PostgresConfig {
    pub fn from_env() -> Self;  // panics only if DATABASE_URL is absent
}
```

---

### `health_check` — readiness probe

```rust
/// Issues `SELECT 1` against the pool. Exercises connection acquisition.
pub async fn health_check(pool: &PgPool) -> Result<(), StorageError>;
```

---

## 📦 Integration & Usage

### Cargo.toml

```toml
[dependencies]
postgres = { path = "crates/shared/storage/postgres" }
```

### Standard bootstrap pattern

```rust
use postgres::{PostgresConfig, PgPoolBuilder, TransactionManager};
use telemetry::TelemetryConfig;

#[tokio::main]
async fn main() {
    // 1. Boot telemetry first — installs the global tracing subscriber that
    //    the postgres tracing hooks will write into.
    let _guard = telemetry::init(
        TelemetryConfig::from_env("account-command-server", env!("CARGO_PKG_VERSION"))
    ).expect("telemetry init failed");

    // 2. Build the pool.
    let pool = PgPoolBuilder::build(PostgresConfig::from_env())
        .await
        .expect("database connection failed");

    // 3. Wrap in TransactionManager for CQRS handlers.
    let tx_mgr = TransactionManager::new(pool.clone());

    // 4. Inject into the command bus / DI container.
    //    tx_mgr.clone() is O(1).
    let bus = build_command_bus(tx_mgr);

    // 5. Optional: mount the health check.
    let router = axum::Router::new()
        .route("/healthz/ready", axum::routing::get({
            let pool = pool.clone();
            move || async move {
                postgres::health::health_check(&pool)
                    .await
                    .map(|_| axum::http::StatusCode::OK)
                    .unwrap_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)
            }
        }));
}
```

### Usage inside a CQRS command handler

```rust
use error::AppError;
use postgres::{StorageError, TransactionManager};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CreateAccountError {
    #[error("account already exists")]
    AlreadyExists,
    #[error(transparent)]
    Storage(#[from] StorageError),  // ← satisfies E: From<StorageError>
}

pub async fn handle_create_account(
    tx_mgr: &TransactionManager,
    cmd: CreateAccountCommand,
) -> Result<AccountId, CreateAccountError> {
    tx_mgr
        .run(|tx| {
            Box::pin(async move {
                let id = sqlx::query_scalar::<_, uuid::Uuid>(
                    "INSERT INTO accounts (id, email) VALUES (gen_random_uuid(), $1) RETURNING id"
                )
                .bind(&cmd.email)
                .fetch_one(&mut **tx)
                .await
                .map_err(|e| match StorageError::from(e) {
                    StorageError::UniqueViolation { .. } => CreateAccountError::AlreadyExists,
                    other => CreateAccountError::Storage(other),
                })?;

                sqlx::query(
                    "INSERT INTO ledger (account_id, balance) VALUES ($1, 0)"
                )
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(StorageError::from)?;  // ? converts via From<StorageError>

                Ok(id)
            })
        })
        .await
}
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | **Yes** | — | libpq connection string. Example: `postgres://user:pass@host:5432/dbname` |
| `PG_MAX_CONNECTIONS` | No | `20` | Hard ceiling on total open connections. Tune based on `max_connections` in `postgresql.conf`. |
| `PG_MIN_CONNECTIONS` | No | `2` | Connections kept warm at all times. Reduces first-request latency. |
| `PG_ACQUIRE_TIMEOUT_SECS` | No | `5` | Seconds to wait for a free pool slot before returning `DB-3001`. |
| `PG_IDLE_TIMEOUT_SECS` | No | `600` | Seconds before an idle connection is closed and removed from the pool. |
| `PG_MAX_LIFETIME_SECS` | No | `1800` | Maximum age of a connection regardless of activity. Prevents stale backend accumulation. |
| `PG_SLOW_STATEMENT_THRESHOLD_MS` | No | `1000` | SQL statements taking longer than this threshold emit a `WARN` tracing event. |

### Cargo feature flags

This crate has no optional feature flags. All functionality is compiled unconditionally.

### Runtime prerequisites

- **Tokio runtime:** `PgPoolBuilder::build` and `TransactionManager::run` are `async`; both must be called from within a `#[tokio::main]` context.
- **Tracing subscriber:** must be installed (via `telemetry::init`) **before** `PgPoolBuilder::build` for sqlx statement logs to be captured. Calling `build` without an active subscriber works but all SQL events are silently discarded.
- **PostgreSQL ≥ 13** recommended. SQLSTATE codes used for deadlock (`40P01`) and serialization failure (`40001`) are stable across all supported versions.

---

## 📈 Telemetry, Performance & Metrics

### Tracing spans emitted by this crate

| Span name | Trigger | Key attributes |
|---|---|---|
| `postgres.pool.build` | `PgPoolBuilder::build()` | `db.max_connections`, `db.min_connections` |
| `db.transaction` | `TransactionManager::run()` | inherits caller's trace context |
| `postgres.health_check` | `health_check()` | — |

Additionally, **sqlx emits one `log` event per statement** at the configured level. The `telemetry` crate's subscriber captures these via the `tracing_log` bridge, so they appear as children of the active `db.transaction` span in Jaeger/Tempo.

### Recommended Prometheus gauges (instrument in the service binary)

```rust
use opentelemetry::{global, KeyValue};

let meter = global::meter("account-command-server");
let pool_size = meter.u64_observable_gauge("db.pool.size")
    .with_description("Total open connections (idle + active)")
    .build();
let pool_idle = meter.u64_observable_gauge("db.pool.idle")
    .with_description("Idle connections available for immediate use")
    .build();

// Register callbacks that read from the pool
meter.register_callback(&[pool_size.as_any(), pool_idle.as_any()], {
    let pool = pool.clone();
    move |observer| {
        let labels = &[KeyValue::new("db.name", "postgres")];
        observer.observe_u64(&pool_size, pool.size() as u64, labels);
        observer.observe_u64(&pool_idle, pool.num_idle() as u64, labels);
    }
}).unwrap();
```

### Recommended production alerts

| Alert | Condition | Severity |
|---|---|---|
| Pool saturation | `db.pool.idle == 0` for > 30 s | Page |
| High acquire-timeout rate | `DB-3001` error code rate > 1/min | Page |
| Deadlock surge | `DB-2001` rate > 5/min (sustained) | Page |
| Slow query budget | P95 statement latency > `PG_SLOW_STATEMENT_THRESHOLD_MS` | Warn |
| Migration failure | Any `DB-5001` event | Page immediately |
| Pool closed unexpectedly | Any `DB-3002` event | Page immediately |

---

## 🛠️ Local Development & Contribution

### Prerequisites

```bash
# Start a local PostgreSQL instance (adjust port if needed)
docker run --rm -d \
  --name pg-dev \
  -e POSTGRES_PASSWORD=postgres \
  -p 5432:5432 \
  postgres:16-alpine

export DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres
```

### Build & check

```bash
# Compile
cargo build -p postgres

# Type-check without linking (fast iteration)
cargo check -p postgres

# Lint
cargo clippy -p postgres -- -D warnings

# Format
cargo fmt -p postgres
```

### Tests

```bash
# Unit tests only (no database required)
cargo test -p postgres

# Unit + integration tests (DATABASE_URL must be set)
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test -p postgres

# Integration tests are standard #[tokio::test] functions;
# they create TEMP TABLEs and clean up automatically.
```

### Adding a new `StorageError` variant

1. Add the variant to `src/error/map.rs` with a **new unique** `DB-XXXX` code.
2. Add all five `AppError` match arms: `error_code`, `http_status`, `severity`, `is_retryable`, `user_facing_message`.
3. Add the `From<sqlx::Error>` arm if the variant corresponds to a new SQLSTATE code.
4. Add a test in `tests/error_mapping.rs` asserting the new code, severity, and retryability.
5. Update the error code table in this README.

---

## 🚨 Troubleshooting & Runbook

### 1. `DB-3001 PoolTimedOut` spikes under load

**Symptom:** Command handlers return `StorageError::PoolTimedOut` and the pool idle gauge drops to 0.

**Root causes:**
- `PG_MAX_CONNECTIONS` is below the number of concurrent handlers.
- A handler is holding a `PgTransaction` open across a long I/O wait (e.g., an HTTP call inside `tx_mgr.run`).
- PostgreSQL's own `max_connections` is lower than `PG_MAX_CONNECTIONS` — the pool has connections in flight but PostgreSQL is rejecting new ones.

**Mitigation:**
1. Check `SELECT count(*) FROM pg_stat_activity` to confirm backend count vs. PostgreSQL's `max_connections`.
2. Audit handler code: **never** perform network I/O, cache lookups, or external service calls inside `tx_mgr.run`. Acquire all data before opening the transaction.
3. Raise `PG_MAX_CONNECTIONS` incrementally and watch `db.pool.idle`.

---

### 2. SQL events do not appear in distributed traces

**Symptom:** `db.transaction` spans appear in Jaeger/Tempo, but child SQL events (from sqlx) are missing.

**Root cause:** `telemetry::init()` was not called before `PgPoolBuilder::build()`, or the `tracing_log` bridge is not active in the subscriber.

**Mitigation:**
1. Ensure `telemetry::init()` is the **first** call in `main()`, before any `sqlx` operation.
2. Verify the `telemetry` crate is wiring the log bridge. The `telemetry` crate's log layer captures sqlx events when `tracing-subscriber`'s `EnvFilter` passes them.
3. Set `RUST_LOG=sqlx=debug` locally to confirm sqlx is emitting log records.

---

### 3. Integration tests fail with "connection refused"

**Symptom:** `tests/transaction_rollback.rs` panics with `failed to connect to the test database`.

**Root cause:** `DATABASE_URL` is not set, or the PostgreSQL container is not running.

**Mitigation:**

```bash
# Start the container
docker run --rm -d --name pg-dev \
  -e POSTGRES_PASSWORD=postgres -p 5432:5432 postgres:16-alpine

# Wait for readiness
until docker exec pg-dev pg_isready -U postgres; do sleep 1; done

# Run tests
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres \
  cargo test -p postgres
```
