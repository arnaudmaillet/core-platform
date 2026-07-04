# `redis-storage` — Instrumented, topology-agnostic Redis client on the `fred` driver

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `storage` — Redis transport/connection capability (no keys, TTLs, or scripts) |
> | **Package** | `redis-storage` (dir: `crates/storage/redis`) |
> | **Consumed by** | `chat`, `profile`, `social-graph`, `engagement`, `geo-discovery`, `notification`, `timeline` |
> | **Depends on** | `fred` 10.x, `telemetry`, `error`, `health` |
> | **Stability** | stable contract |
> | **Feature flags** | inherits fred features (e.g. `subscriber-client` for `SSUBSCRIBE`/`SPUBLISH`) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`redis-storage` is the shared Redis infrastructure crate: a production-grade, fully-instrumented client
abstraction over the [`fred`](https://crates.io/crates/fred) driver (v10.x), wiring automatic
multiplexing, topology agnosticism, exponential-backoff reconnection, OTel-native telemetry, and an
`AppError`-compatible error type into one reusable primitive. Consumers import `RedisClient` /
`RedisPool` and use fred's command traits directly (via `Deref`).

**Architectural boundary** — it contains **zero** application cache keys, TTLs, domain models, Lua
scripts, or rate-limit logic. It exposes only the transport and connection capability.

---

## 📐 Architecture & key decisions

```
Consumers (CQRS, cache utils, rate-limit mw) ── RedisClient / RedisPool
        ▼
redis-storage: config(topology) · error(map) · listener(event) · client/pool(builder) · health(check)
        ▼  fred::types::Builder
fred 10.x (multiplexer, pipeline engine, reconnect policy)  ──TCP/TLS──► Cluster / Sentinel / Standalone
```

- **One multiplexer per client** — fred routes *all* commands from *all* callers through a single
  lock-free background writer, so there's no per-command lock; same-tick commands auto-pipeline into
  one flush (`REDIS_AUTO_PIPELINE`). A `RedisPool` of size N is N independent multiplexers for
  write-bandwidth-bound workloads.
- **Topology behind one function** — `TopologyKind::into_server_config()` is the single translation of
  env config into fred's `ServerConfig` (`standalone`→Centralized, `cluster`→Clustered,
  `sentinel`→Sentinel). Adding a topology touches only that enum + function.
- **Errors mapped to stable `RDS-xxxx`** — every `fred::error::RedisError` collapses to a named
  `RedisStorageError` variant with code, `Severity`, retryability, and HTTP status, so consumers branch
  on the platform contract, not fred internals.
- **Telemetry bridged at construction** — the builders spawn an event listener bridging fred's
  connect/reconnect/error lifecycle to the process-global OTel subscriber, so it **must** be installed
  *before* `build()`.

---

## 🔌 Public API & contract

```rust
pub struct RedisClient { pub inner: fred::clients::RedisClient }   // single multiplexed connection
pub struct RedisPool   { pub inner: fred::clients::Pool }          // N connections (throughput-critical)
impl Deref for RedisClient/RedisPool { /* → fred client; use command traits directly */ }

pub struct RedisClientBuilder; impl { pub fn new(RedisConfig) -> Self; pub async fn build(self) -> Result<RedisClient, RedisStorageError>; }
pub struct RedisPoolBuilder;   impl { pub fn new(RedisConfig) -> Self; pub async fn build(self) -> Result<RedisPool, RedisStorageError>; }

pub async fn health_check<C: ClientLike + HeartbeatInterface>(client: &C) -> Result<(), RedisStorageError>;
pub fn spawn_event_listener<C: EventInterface>(client: &C) -> [JoinHandle<()>; 3];   // called by builders
```

> **Contract notes:** clients/pools `Deref` to the fred client — call fred command traits
> (`KeysInterface`, `HashesInterface`, …) directly. `RedisClient` is cheaply cloneable. The OTel
> subscriber must be installed before `build()` (fred emits spans at construction).

---

## 🧯 Error model

`RedisStorageError` (`#[non_exhaustive]`) implements `error::AppError`; category is always `"RDS"`:

| Code | Variant | Retryable | Severity | HTTP |
|---|---|---|---|---|
| RDS-1001 | `Timeout` | yes | High | 503 |
| RDS-1002 | `Disconnected` | yes | High | 503 |
| RDS-1003 | `Io` | yes | High | 503 |
| RDS-1004 | `Backpressure` | yes | High | 503 |
| RDS-1005 | `Canceled` | yes | Medium | 503 |
| RDS-2001 | `PoolExhausted` | yes | High | 503 |
| RDS-3001 | `Authentication` | no | Critical | 500 |
| RDS-4001 | `WrongType` | no | Low | 422 |
| RDS-4002 | `InvalidArgument` | no | Low | 422 |
| RDS-4003 | `InvalidCommand` | no | Medium | 500 |
| RDS-4004 | `NotFound` | no | Low | 404 |
| RDS-5001 | `Cluster` | yes | High | 503 |
| RDS-7001 | `Sentinel` | yes | High | 503 |
| RDS-8001..8004 | `Configuration`/`Tls`/`Protocol`/`Parse` | no | Crit/Crit/Crit/Medium | 500 |
| RDS-9000 | `Unknown` | no | Medium | 500 |

---

## 📦 Integration

```toml
[dependencies]
redis-storage = { workspace = true }
```

```rust
use fred::interfaces::{KeysInterface, HashesInterface};
use redis_storage::{RedisConfig, RedisPoolBuilder, health::health_check};

telemetry::init(telemetry::Config::from_env()).await?;          // BEFORE build — fred emits into the subscriber
let pool = RedisPoolBuilder::new(RedisConfig::from_env()).build().await?;
health_check(&pool).await?;
pool.set::<(), _, _>("session:42", "payload", None, None, false).await?;  // fred traits via Deref
```

---

## ⚙️ Configuration & feature flags

**Connection / topology:** `REDIS_TOPOLOGY` (`standalone`|`cluster`|`sentinel`, default `standalone`),
`REDIS_HOSTS` (default `127.0.0.1:6379`), `REDIS_USERNAME`/`REDIS_PASSWORD`, `REDIS_DATABASE` (0–15,
ignored in cluster). **Sentinel:** `REDIS_SENTINEL_SERVICE_NAME` (default `mymaster`) +
`REDIS_SENTINEL_USERNAME`/`PASSWORD`. **Tuning:** `REDIS_CONNECTION_TIMEOUT_SECS` (5),
`REDIS_COMMAND_TIMEOUT_MS` (3000; 0 disables), `REDIS_FAIL_FAST` (true),
`REDIS_UNRESPONSIVE_TIMEOUT_SECS` (60). **Pool:** `REDIS_POOL_SIZE` (8). **Pipelining:**
`REDIS_AUTO_PIPELINE` (true), `REDIS_PIPELINE_BATCH_SIZE` (200), `REDIS_MAX_COMMAND_BUFFER_LEN`
(10000). **Reconnect:** `REDIS_RECONNECT_MIN/MAX_DELAY_MS` (100 / 30000), `REDIS_RECONNECT_MAX_ATTEMPTS`
(0 = unlimited), `REDIS_RECONNECT_MULTIPLIER` (2). **Cluster:** `REDIS_MAX_REDIRECTIONS` (fred default 16).

**Feature flags:** inherits fred's — notably `subscriber-client` (transitively enabled for services
using sharded pub/sub `SSUBSCRIBE`/`SPUBLISH`, e.g. `chat`).

---

## 🔭 Observability

fred emits `fred.command` spans (`DEBUG`, `db.system=redis`, `net.peer.*`). The event listener emits
connect/reconnect (`INFO`) and connection-error (`ERROR`, `error.message`) events, all
`otel.kind=CLIENT`.

Suggested alerts: `RDS-1001` rate > 10/5m ⇒ high (network); any `RDS-3001` ⇒ critical (creds);
sustained `RDS-2001` ⇒ high (pool undersized); `RDS-5001` spikes ⇒ high (cluster failover).

---

## 🧪 Testing

```bash
cargo test   -p redis-storage                 # unit — no Redis
cargo clippy -p redis-storage --all-targets
# integration (live):
REDIS_HOSTS=127.0.0.1:6379 cargo test -p redis-storage -- --include-ignored
REDIS_TOPOLOGY=cluster REDIS_HOSTS=127.0.0.1:7000,7001,7002 cargo test -p redis-storage -- --include-ignored
```

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `RedisStorageError::Configuration` at startup — empty host list.**
`REDIS_HOSTS` is empty/whitespace. `from_env()` asserts at least one valid `host:port`. Set
`REDIS_HOSTS=127.0.0.1:6379`.

**2. `Disconnected` with `fail_fast = true` — service won't start.**
Redis wasn't reachable at boot (race with container health, or it's down). Add a Redis readiness probe
+ `depends_on`, or set `REDIS_FAIL_FAST=false` to enter the reconnect loop instead of failing, or raise
`REDIS_CONNECTION_TIMEOUT_SECS` on high-RTT networks.

**3. Sustained `RDS-2001 PoolExhausted` under load.**
`REDIS_POOL_SIZE` too small (each member is one TCP connection). Raise it (16→32, profiling server-side
conn count first), raise `REDIS_MAX_COMMAND_BUFFER_LEN` to absorb spikes, confirm
`REDIS_AUTO_PIPELINE=true` (amortises RTT). If Redis CPU is the bottleneck, scale Redis horizontally.

**4. No spans for my commands, or lifecycle events missing.**
The OTel subscriber wasn't installed before `build()`. Call `telemetry::init(...)` first — fred wires
its tracing hooks into the *active* subscriber at construction time.
