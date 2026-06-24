# redis-storage — High-performance Redis client abstraction for hyperscale social infrastructure

## 🎯 Overview & Service Role

`redis-storage` is the shared Redis infrastructure crate for the `core-platform` workspace. It provides a **production-grade, fully-instrumented Redis client abstraction** built on the [`fred`](https://crates.io/crates/fred) driver (v10.x), wiring automatic multiplexing, topology agnosticism, exponential backoff reconnection, OTel-native telemetry, and a structured `AppError`-compatible error type into a single, reusable primitive.

**Upstream consumers** (CQRS handlers, cache utilities, rate-limit middleware) import `RedisClient` or `RedisPool` and use fred's command traits directly — without ever touching connection lifecycle, backoff logic, or tracing wiring.

**Critical scope boundary:** This crate contains **zero** application-specific cache keys, TTL values, domain models, Lua scripts, or rate-limiting logic. It exposes only the transport and connection capability.

### Core technical objectives

- **Zero-cost multiplexing** via fred's lock-free, single-connection command queue — thousands of concurrent commands over one TCP socket without a semaphore.
- **Topology agnosticism** — switch between Standalone / Redis Cluster / Redis Sentinel via a single environment variable with no code change.
- **Full OTel integration** — fred's built-in tracing feature emits command-level spans; a dedicated event listener bridges connection lifecycle events (connect / reconnect / error) to the process-global OTel subscriber.
- **Structured error propagation** — every `fred::error::RedisError` maps to a named `RedisStorageError` variant with a stable `RDS-xxxx` code, `Severity`, retryability flag, and HTTP status that integrate directly with the platform's `AppError` pipeline.

---

## 📐 Architecture & Concepts

### Layered architecture

```
┌─────────────────────────────────────────────────────┐
│               Upstream Consumers                     │
│  (CQRS handlers, cache utils, rate-limit middleware) │
└──────────────────┬──────────────────────────────────┘
                   │ RedisClient / RedisPool
┌──────────────────▼──────────────────────────────────┐
│                redis-storage                         │
│                                                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
│  │   config/   │  │   error/    │  │  listener/  │  │
│  │ connection  │  │    map      │  │    event    │  │
│  │  topology   │  │             │  │             │  │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  │
│         │                │                │          │
│  ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼──────┐  │
│  │   client/   │  │    pool/    │  │   health/   │  │
│  │   builder   │  │   builder   │  │    check    │  │
│  └─────────────┘  └─────────────┘  └─────────────┘  │
└──────────────────────────────────────────────────────┘
                   │ fred::types::Builder
┌──────────────────▼──────────────────────────────────┐
│              fred 10.x driver                        │
│  (multiplexer, pipeline engine, reconnect policy)    │
└──────────────────────────────────────────────────────┘
                   │ TCP / TLS
┌──────────────────▼──────────────────────────────────┐
│         Redis Cluster / Sentinel / Standalone        │
└──────────────────────────────────────────────────────┘
```

### fred multiplexing model

Fred routes **all commands from all callers through a single background writer task** per `RedisClient`. This means:

- No per-command lock contention — the command is atomically pushed onto a lock-free queue.
- Auto-pipelining (`REDIS_AUTO_PIPELINE=true`) batches commands that arrive within the same scheduler tick into one `PIPELINE` flush, amortising RTT across concurrent callers.
- A `RedisPool` of size `N` creates `N` independent multiplexers for workloads that saturate one connection's write bandwidth.

### Topology agnosticism

`TopologyKind::into_server_config()` is the single point that translates our environment-driven config into fred's `ServerConfig` enum. Adding a new topology (e.g., `RedisStack`, `ElastiCache`) requires only extending `TopologyKind` and this one function — all builders, error mapping, and telemetry remain unchanged.

```
REDIS_TOPOLOGY=standalone  →  ServerConfig::Centralized { server: Server }
REDIS_TOPOLOGY=cluster     →  ServerConfig::Clustered   { hosts: Vec<Server> }
REDIS_TOPOLOGY=sentinel    →  ServerConfig::Sentinel    { hosts, service_name, ... }
```

### Resilience guarantees & high-load behaviour

| Concern | Mechanism |
|---|---|
| **Transient disconnects** | Exponential backoff reconnect policy (`ReconnectPolicy::new_exponential`). Configurable min/max delay, multiplier, max attempts. |
| **Command timeout** | Per-command deadline via `PerformanceConfig::default_command_timeout`. Surfaces as `RDS-1001 Timeout`. |
| **Backpressure** | Internal command buffer bounded by `REDIS_MAX_COMMAND_BUFFER_LEN`. When full, fred returns `RedisErrorKind::Backpressure` → `RDS-1004`. Callers should retry with a brief delay. |
| **Cluster topology change** | fred's cluster client follows MOVED/ASK redirects automatically. Limit configurable via `REDIS_MAX_REDIRECTIONS`. |
| **Sentinel failover** | fred promotes the new primary transparently. Brief window of `RDS-7001 Sentinel` errors during election is expected and retryable. |
| **Unresponsive connection** | Detected after `REDIS_UNRESPONSIVE_TIMEOUT_SECS` and replaced automatically by the reconnect policy. |
| **Pool saturation** | When all pool members queue more commands than their buffer allows, callers receive `RDS-2001 PoolExhausted` (retryable with backoff). |

---

## 🔌 Public Interfaces & API Contract

### Core types

```rust
// Single multiplexed connection — sufficient for most services.
pub struct RedisClient {
    pub inner: fred::clients::RedisClient,
}
impl Deref for RedisClient { type Target = fred::clients::RedisClient; }

// Pool of N multiplexed connections — for throughput-critical workloads.
pub struct RedisPool {
    pub inner: fred::clients::Pool,
}
impl Deref for RedisPool { type Target = fred::clients::Pool; }
```

### Builders

```rust
pub struct RedisClientBuilder { /* ... */ }
impl RedisClientBuilder {
    pub fn new(config: RedisConfig) -> Self;
    pub async fn build(self) -> Result<RedisClient, RedisStorageError>;
}

pub struct RedisPoolBuilder { /* ... */ }
impl RedisPoolBuilder {
    pub fn new(config: RedisConfig) -> Self;
    pub async fn build(self) -> Result<RedisPool, RedisStorageError>;
}
```

### Error type

```rust
#[non_exhaustive]
pub enum RedisStorageError {
    Timeout { message: String },          // RDS-1001  retryable  High    503
    Disconnected { message: String },     // RDS-1002  retryable  High    503
    Io { message: String },              // RDS-1003  retryable  High    503
    Backpressure,                         // RDS-1004  retryable  High    503
    Canceled,                             // RDS-1005  retryable  Medium  503
    PoolExhausted { message: String },    // RDS-2001  retryable  High    503
    Authentication { message: String },   // RDS-3001  permanent  Crit    500
    WrongType { message: String },        // RDS-4001  permanent  Low     422
    InvalidArgument { message: String },  // RDS-4002  permanent  Low     422
    InvalidCommand { message: String },   // RDS-4003  permanent  Medium  500
    NotFound,                             // RDS-4004  permanent  Low     404
    Cluster { message: String },          // RDS-5001  retryable  High    503
    Sentinel { message: String },         // RDS-7001  retryable  High    503
    Configuration { message: String },    // RDS-8001  permanent  Crit    500
    Tls { message: String },             // RDS-8002  permanent  Crit    500
    Protocol { message: String },         // RDS-8003  permanent  Crit    500
    Parse { message: String },            // RDS-8004  permanent  Medium  500
    Unknown { message: String },          // RDS-9000  permanent  Medium  500
}
```

All variants implement `error::AppError`:

```rust
pub trait AppError {
    fn error_code(&self)          -> &'static str;   // e.g. "RDS-1001"
    fn http_status(&self)         -> StatusCode;
    fn severity(&self)            -> Severity;        // Critical / High / Medium / Low
    fn is_retryable(&self)        -> bool;
    fn category(&self)            -> &'static str;   // always "RDS"
    fn user_facing_message(&self) -> &'static str;
}
```

### Health check

```rust
// Works with RedisClient, RedisPool, or any fred ClientLike + HeartbeatInterface
pub async fn health_check<C: ClientLike + HeartbeatInterface>(
    client: &C,
) -> Result<(), RedisStorageError>;
```

### Event listener

```rust
// Spawns 3 background tasks bridging fred lifecycle streams → tracing spans.
// Called automatically by the builders; exposed for advanced usage.
pub fn spawn_event_listener<C: EventInterface>(client: &C) -> [JoinHandle<()>; 3];
```

---

## 📦 Integration & Usage

### Dependency declaration

```toml
# Cargo.toml
[dependencies]
redis-storage = { workspace = true }
```

### Standard bootstrap — pool (recommended for production)

```rust
use fred::interfaces::{KeysInterface, HashesInterface};
use redis_storage::{RedisConfig, RedisPoolBuilder, health::health_check};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. The telemetry crate must install its subscriber BEFORE building
    //    the pool — fred's tracing hooks emit into the active subscriber.
    telemetry::init(telemetry::Config::from_env()).await?;

    // 2. Load config from environment variables.
    let config = RedisConfig::from_env();

    // 3. Build the pool. This:
    //    - Resolves topology (REDIS_TOPOLOGY → ServerConfig variant)
    //    - Connects all pool members (REDIS_POOL_SIZE connections)
    //    - Spawns the OTel event listener
    let pool = RedisPoolBuilder::new(config).build().await?;

    // 4. Optional startup liveness check.
    health_check(&pool).await?;

    // 5. Use fred command traits directly on pool (via Deref).
    pool.set::<(), _, _>("session:42", "payload", None, None, false).await?;
    let val: Option<String> = pool.get("session:42").await?;

    // 6. Hash operations example (CQRS command handler pattern).
    pool.hset::<(), _, _>("user:profile:42", vec![
        ("name", "Alice"),
        ("score", "9500"),
    ]).await?;

    Ok(())
}
```

### Lightweight single-client bootstrap

```rust
use redis_storage::{RedisConfig, RedisClientBuilder};

let config = RedisConfig::from_env();
let client = RedisClientBuilder::new(config).build().await?;

// client is cheaply cloneable — pass Arc<RedisClient> or clone into tasks.
let client_clone = client.clone();
tokio::spawn(async move {
    let _: Option<String> = client_clone.get("live:counter").await.ok().flatten();
});
```

---

## ⚙️ Configuration & Runtime Environment

### Connection & topology

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_TOPOLOGY` | No | `standalone` | Deployment topology: `standalone` \| `cluster` \| `sentinel` |
| `REDIS_HOSTS` | No | `127.0.0.1:6379` | Comma-separated `host:port` list. All entries used for cluster/sentinel; only first for standalone. |
| `REDIS_USERNAME` | No | — | ACL username for data-plane AUTH. |
| `REDIS_PASSWORD` | No | — | Password for data-plane AUTH. |
| `REDIS_DATABASE` | No | `0` | Database index (0–15). Ignored in cluster mode. |

### Sentinel-specific

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_SENTINEL_SERVICE_NAME` | No | `mymaster` | Logical name of the Sentinel-managed primary. |
| `REDIS_SENTINEL_USERNAME` | No | — | Username for authenticating to Sentinel nodes. |
| `REDIS_SENTINEL_PASSWORD` | No | — | Password for authenticating to Sentinel nodes. |

### Connection tuning

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_CONNECTION_TIMEOUT_SECS` | No | `5.0` | TCP handshake deadline per node. |
| `REDIS_COMMAND_TIMEOUT_MS` | No | `3000` | Per-command response deadline. `0` disables timeout. |
| `REDIS_FAIL_FAST` | No | `true` | When `true`, startup fails immediately if Redis is unreachable. |
| `REDIS_UNRESPONSIVE_TIMEOUT_SECS` | No | `60.0` | Seconds before an unresponsive connection is replaced. |

### Pool

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_POOL_SIZE` | No | `8` | Number of independent `RedisClient` connections in the pool. |

### Pipelining

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_AUTO_PIPELINE` | No | `true` | Batch same-tick commands into one pipeline flush. |
| `REDIS_PIPELINE_BATCH_SIZE` | No | `200` | Maximum commands per pipeline flush. |
| `REDIS_MAX_COMMAND_BUFFER_LEN` | No | `10000` | Internal command queue depth before backpressure fires. |

### Reconnect (exponential backoff)

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_RECONNECT_MIN_DELAY_MS` | No | `100` | Initial reconnect delay in milliseconds. |
| `REDIS_RECONNECT_MAX_DELAY_MS` | No | `30000` | Maximum reconnect delay cap in milliseconds. |
| `REDIS_RECONNECT_MAX_ATTEMPTS` | No | `0` | Maximum reconnect attempts. `0` = unlimited. |
| `REDIS_RECONNECT_MULTIPLIER` | No | `2` | Exponential multiplier applied per failed attempt. |

### Cluster

| Variable | Required | Default | Description |
|---|---|---|---|
| `REDIS_MAX_REDIRECTIONS` | No | fred default (16) | Maximum MOVED/ASK redirects per command before error. |

---

## 📈 Telemetry, Performance & Metrics

### Prerequisites

- A Tokio `rt-multi-thread` runtime must be active before calling `build()` — the event listener spawns background tasks via `tokio::spawn`.
- The `telemetry` crate must install its OTel subscriber **before** building the client or pool. Fred emits tracing spans into the active subscriber at construction time.

### OTel spans emitted by fred (via `tracing` feature)

| Span name | Level | Key fields |
|---|---|---|
| `fred.command` | `DEBUG` | `db.operation`, `db.system=redis`, `net.peer.name`, `net.peer.port` |

### Connection lifecycle events (emitted by `listener/event.rs`)

| Event | Level | Key fields |
|---|---|---|
| Redis connect | `INFO` | `db.system=redis`, `otel.kind=CLIENT` |
| Redis reconnect | `INFO` | `db.system=redis`, `otel.kind=CLIENT` |
| Redis connection error | `ERROR` | `db.system=redis`, `otel.kind=CLIENT`, `error=true`, `error.message` |

### Recommended Prometheus/OTel alerts

| Alert | Condition | Severity |
|---|---|---|
| High RDS-1001 rate | `rate(rds_errors_total{code="RDS-1001"}[5m]) > 10` | High — likely network instability |
| Any RDS-3001 | `rds_errors_total{code="RDS-3001"} > 0` | Critical — credential misconfiguration |
| RDS-2001 sustained | `rate(rds_errors_total{code="RDS-2001"}[5m]) > 5` | High — pool undersized for load |
| RDS-5001 spikes | `rate(rds_errors_total{code="RDS-5001"}[1m]) > 20` | High — cluster failover in progress |

---

## 🛠️ Local Development & Contribution

### Start a local Redis instance

```bash
# Standalone (default)
docker compose up -d redis

# Redis Cluster (6-node, 3 primary + 3 replica)
docker compose -f docker-compose.cluster.yml up -d

# Sentinel (1 primary + 2 replicas + 3 sentinels)
docker compose -f docker-compose.sentinel.yml up -d
```

### Build & lint

```bash
cargo build -p redis-storage
cargo clippy -p redis-storage -- -D warnings
cargo fmt --check -p redis-storage
```

### Unit tests (no Redis required)

```bash
cargo test -p redis-storage
```

### Integration tests (live Redis required)

```bash
# Standalone
REDIS_HOSTS=127.0.0.1:6379 \
  cargo test -p redis-storage -- --include-ignored

# Cluster
REDIS_TOPOLOGY=cluster \
REDIS_HOSTS=127.0.0.1:7000,127.0.0.1:7001,127.0.0.1:7002 \
  cargo test -p redis-storage -- --include-ignored

# Sentinel
REDIS_TOPOLOGY=sentinel \
REDIS_HOSTS=127.0.0.1:26379,127.0.0.1:26380,127.0.0.1:26381 \
REDIS_SENTINEL_SERVICE_NAME=mymaster \
  cargo test -p redis-storage -- --include-ignored
```

---

## 🚨 Troubleshooting & Runbook (FAQ)

### 1. `RedisStorageError::Configuration` at startup — empty host list

**Root cause:** `REDIS_HOSTS` is set to an empty string, or only whitespace.

**Fix:** Ensure `REDIS_HOSTS` contains at least one valid `host:port` entry. The crate asserts non-emptiness at `from_env()` time.

```bash
export REDIS_HOSTS=127.0.0.1:6379
```

---

### 2. `RedisStorageError::Disconnected` with `fail_fast = true` — service fails to start

**Root cause:** Redis is not yet reachable when the service starts (race with container health checks, or Redis is genuinely down).

**Mitigations:**
- Add a readiness probe on the Redis container and set `depends_on` with health checks in Docker Compose.
- Set `REDIS_FAIL_FAST=false` to enter the reconnect loop instead of failing immediately — appropriate when Redis startup may lag behind the application.
- Increase `REDIS_CONNECTION_TIMEOUT_SECS` if the network RTT to Redis is high.

---

### 3. Sustained `RDS-2001 PoolExhausted` under load

**Root cause:** `REDIS_POOL_SIZE` is too small for the command throughput. Each `RedisClient` in the pool is a single TCP connection; when the write bandwidth of all connections is saturated, backpressure propagates to callers.

**Mitigations:**
1. Increase `REDIS_POOL_SIZE` (try `16` → `32`). Profile server-side connection count and memory before going higher.
2. Increase `REDIS_MAX_COMMAND_BUFFER_LEN` to absorb short traffic spikes without surfacing as pool errors.
3. Verify that `REDIS_AUTO_PIPELINE=true` — this amortises RTT and increases effective throughput per connection significantly.
4. If the bottleneck is Redis CPU rather than connection bandwidth, scale Redis horizontally (add shards / cluster nodes).
