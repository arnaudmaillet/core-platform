<!--
================================================================================
 FLEET SERVICE README — STANDARD TEMPLATE
================================================================================
 Copy this file to crates/services/<service-name>/README.md and fill every
 <placeholder>. Delete these HTML comments and any section the tier rule below
 marks optional and that genuinely does not apply.

 PLACEHOLDER CONVENTION
   <service-name>   crate/dir name ............ e.g. chat
   <Service>        Rust/proto type ........... e.g. ChatService
   <SVC>            env-var prefix (UPPER) .... e.g. CHAT
   <SVC-CODE>       error-code prefix ......... e.g. CHT
   <...>            everything else

 TIER RULE (how much to fill)
   CORE — always required, every service, every tier:
     Service Card · Overview · Architecture · Public Interfaces · Events
     · Integration · Configuration · Local Development · Troubleshooting
   OPS — full prose for TIER-0 / TIER-1 (critical path); a one-line "N/A —
   <reason>" is acceptable for TIER-2 / best-effort / derived services:
     SLO · Dependencies & Blast Radius · Failure Modes · Deployment
   Do not DELETE ops sections on Tier-2 — collapse them to one honest line so
   the service catalog stays uniform and grep-able.

 VOICE
   This is a runbook and an engineering contract, not an essay. Every line
   should constrain a caller or an operator. If a sentence informs neither,
   cut it. Prefer tables and "symptom → root cause → mitigation" over prose.
================================================================================
-->

# `<service-name>` — <one-line value prop: name the HARD problem this service exists to solve>

<!-- Tagline rule: name the hard problem, not the feature. If your tagline could
     describe a tutorial CRUD app, rewrite it until it couldn't. -->

> **Service Card** &nbsp;·&nbsp; CORE
>
> | | |
> |---|---|
> | **Owner** | `<team>` · `<#slack-channel>` |
> | **On-call / escalation** | `<oncall-rotation>` → `<escalation-policy>` |
> | **Tier** | `<TIER-0 critical-path \| TIER-1 \| TIER-2 best-effort>` |
> | **Deployable** | `crates/apps/<service-name>-server` (library crate: `crates/services/<service-name>`) |
> | **Datastores** | `<ScyllaDB keyspace `name` \| Postgres db `name` \| Redis Cluster>` |
> | **Async** | publishes `<topic.*>` · consumes `<topic.*>` (Kafka) |
> | **Upstream callers** | `<service-a>`, `<gateway>` |
> | **Downstream deps** | `<service-x>`, `<store>`, Redis, Kafka |
> | **SLO** | `<99.9%>` avail · `<p99 read < N ms>` · `<p99 write < N ms>` |

---

## 🎯 Overview & Service Role

`<service-name>` is the **<Bounded Context / Aggregate>** microservice for the platform: it owns
<the data and invariants it is the single source of truth for>.

The hard problem it solves is **<the core engineering tension>** — <2–3 sentences: the load
shape, the failure a naive design hits, and the named pattern that resolves it>.

**Core objectives:** <the 2–3 non-negotiable guarantees the rest of the doc must uphold>.

<!-- Optional: a small comparison table when the service has two distinct modes/planes. -->

---

## 📐 Architecture & Concepts

Hexagonal / DDD (`domain` → `application` → `infrastructure`), CQRS command/query buses,
`<ScyllaDB | Postgres>` for the durable store, Redis for `<cache | real-time routing>`,
Kafka for events.

```
<ASCII diagram — one screen max. Show the data flow that explains the hard problem,
 not every struct. ingress (gRPC) → CQRS bus → ports → adapters (store / cache / events).>
```

**<Key mechanism>.** <The non-obvious core: the partition strategy, sharding scheme, or
idempotency model a reviewer MUST understand to avoid breaking the service.>

> **Invariants** (and where enforced): <list each domain invariant and the layer that owns it,
> e.g. "membership checked at the gRPC boundary", "counter mutations Lua-atomic in Redis">.

---

## 📊 Service Level Objectives (SLO) &nbsp;·&nbsp; OPS

<!-- TIER-0/1: fill the table. TIER-2: replace with one line, e.g.
     "N/A — derived/async service, no synchronous SLO; tracked via consumer lag only." -->

| SLI | Objective | Window | Measured by |
|---|---|---|---|
| Availability (non-5xx / non-`UNAVAILABLE`) | `<99.9%>` | 30d rolling | `<metric>` |
| Read latency p99 | `< <N> ms` | 1h | `<metric>` (by consistency profile) |
| Write latency p99 | `< <N> ms` | 1h | `<metric>` |
| Async lag (consumer) | `< <N> s` | live | `<consumer-group> lag` |
| Durability | `<no acked write lost>` | — | `<LocalQuorum / fsync policy>` |

**Error budget:** `<0.1% / 30d ≈ 43m>`. **On burn:** `<freeze rollout \| page>`.

---

## 🔗 Dependencies & Blast Radius &nbsp;·&nbsp; OPS

**Downstream — what `<service-name>` needs to function:**

| Dependency | Purpose | If down → | Degradation |
|---|---|---|---|
| `<ScyllaDB>` | durable store | `<writes fail>` | **Hard** — `<UNAVAILABLE>` |
| `<Redis>` | `<cache / real-time>` | `<cache-miss path>` | **Soft** — `<falls back to store>` |
| `<Kafka>` | event emission | `<events not emitted>` | **Soft** — `<best-effort, logged>` |
| `<service-x>` (gRPC) | `<reason>` | `<which RPCs fail>` | `<Hard / Soft>` |

**Upstream — who depends on `<service-name>` (your blast radius if YOU fail):**

| Caller | Uses | User-visible impact if `<service-name>` is down |
|---|---|---|
| `<service-a>` | `<RPC / topic>` | `<effect>` |

> **Critical path?** `<Yes — in the synchronous request path of X \| No — async/derived>`.

---

## 🔌 Public Interfaces & API Contract &nbsp;·&nbsp; CORE

### gRPC — `<package>.v1.<Service>`

```protobuf
service <Service> {
  // <group> — <purpose>
  rpc <Method> (<Req>) returns (<Resp>);
  rpc <Stream> (<Req>) returns (stream <Resp>);
}
```

> **Wire / enum contract:** <the exact mapping rule so codegen consumers can't drift,
> e.g. "proto enums are 0-based and equal the domain tinyint; no UNSPECIFIED sentinel">.

**Boundary invariants:** <which RPC requires which precondition and the exact `Status` on
violation — `PERMISSION_DENIED`, `FAILED_PRECONDITION`, …>.

### Rust ports (hexagonal contract)

```rust
#[async_trait] pub trait <Repository> { /* the stable port the domain depends on */ }
```

### Error contract

Every fault implements `error::AppError` with a stable code, mapped to gRPC `Status` / HTTP
by the shared `error` crate:

| Range | Class |
|---|---|
| `<SVC-CODE>-1xxx` | `<lifecycle>` |
| `<SVC-CODE>-2xxx` | `<validation>` |
| `<SVC-CODE>-9xxx` | `<identifiers / internal>` |

---

## 📨 Events & Async Contract &nbsp;·&nbsp; CORE

> Kafka topics are an API. A schema change here breaks consumers exactly like a proto change.

**Publishes:**

| Topic | Trigger | Key | Consumers |
|---|---|---|---|
| `<topic.created>` | `<when>` | `<partition key>` | `<service-x>` |

**Consumes:**

| Topic | Consumer group | Purpose | On poison/exhaustion |
|---|---|---|---|
| `<topic.in>` | `<service-name>-<role>-consumer` | `<reason>` | DLQ `<topic.dlq>` |

> **Runtime contract (mandatory):** all consumers run under `run_consumer` — manual commit
> after a terminal outcome, bounded retry with backoff + jitter, DLQ on exhaustion/poison,
> rebuild-from-last-committed-offset on broker error. Idempotency: `<deterministic key / UUIDv5 / claim>`.

---

## 🌩️ Failure Modes & Degradation &nbsp;·&nbsp; OPS

<!-- TIER-2 may collapse to a one-line pointer at the Failure column of §Dependencies. -->

| Failure | Symptom | Service behavior | Operator action |
|---|---|---|---|
| `<dep>` unavailable | `<error / latency>` | `<hard fail \| degrade to X>` | `<runbook step>` |
| `<cache>` cold/evicted | `<latency rise>` | `<rebuild from store, safe>` | `<usually none>` |
| Consumer lag growing | `<delayed side effect>` | `<retry within budget>` | `<check broker/dep>` |

**Backpressure & limits:** <how the service sheds load — page-size caps, broadcast buffers,
rate limits — and the knob that tunes each>.

---

## 📦 Integration & Usage &nbsp;·&nbsp; CORE

```toml
[dependencies]
<service-name> = { path = "crates/services/<service-name>" }
```

Library-only. Implements [`service_runtime::Service`](../../platform/service-runtime/README.md)
as `<service_name>::service::<Service>` — `build` wires adapters `<and spawns: ...>`, `register`
adds the gRPC services, `health_probes` exposes liveness. Telemetry, config + hot-reload,
ingress rate-limiting, health, and graceful shutdown are owned by the runtime.

### Bootstrap (`crates/apps/<service-name>-server`)

```rust
use <service_name>::service::<Service>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr = std::env::var("<SVC>_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:<port>".to_owned()).parse()?;
    service_runtime::serve::<<Service>>(addr).await
}
```

---

## ⚙️ Configuration & Runtime Environment &nbsp;·&nbsp; CORE

### `<service-name>`-specific variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `<SVC>_<KNOB>` | No | `<default>` | `<what it tunes; flag "must be uniform cluster-wide" where it matters>` |

### Inherited infrastructure variables

| Variable | Required | Default | Description |
|---|---|---|---|
| `<SCYLLA_* / POSTGRES_* / REDIS_HOSTS / KAFKA_BROKERS>` | **Yes** | — | `<...>` |

> Full connection/timeout/reconnect tuning lives in the relevant shared storage/transport crates.

### Compile-time features
- `<feature>` — `<why required>`
- `build.rs` compiles `<proto path>` and emits the reflection descriptor set.

---

## 🚀 Deployment, Migrations & Rollback &nbsp;·&nbsp; OPS

- **Migrations:** `crates/services/<service-name>/migrations/*.{cql,sql}` against `<keyspace/db>`.
  Apply **before** rolling the new binary. <Expand-then-contract? backward-compatible?>
- **Rollout:** `<rolling / canary>`; `<feature-flag or config gate for risky changes>`.
- **Rollback:** `<safe to roll back binary? migrations forward-compatible with N-1?>`.
- **Stateful gotchas:** `<knobs that must NEVER change after data exists — bucket width, hash scheme>`.

---

## 📈 Telemetry, Performance & Metrics &nbsp;·&nbsp; CORE

- **Runtime:** multi-threaded Tokio (`<streams / reapers / consumers>`). Global tracing/OTel
  subscriber installed before `serve`; W3C trace-context propagated across the Kafka boundary.

| Signal | Why it matters | Suggested alert |
|---|---|---|
| `<metric>` | `<what it tells you>` | `<condition ⇒ action>` |
| DLQ produce rate (`<topic.dlq>`) | poison / retry-exhausted | any sustained rate ⇒ page |

---

## 🛠️ Local Development &nbsp;·&nbsp; CORE

```bash
cargo build -p <service-name> && cargo clippy -p <service-name> --all-targets
cargo test  -p <service-name>
docker compose up -d <scylla redis kafka>        # repo-root compose
for f in crates/services/<service-name>/migrations/*; do <apply>; done
```

---

## 🚨 Troubleshooting & Runbook &nbsp;·&nbsp; CORE

> Format: **symptom → root cause → mitigation.** One entry per real incident class.

**1. `<exact error a caller sees>`.**
Root cause: `<the actual mechanism>`. Mitigation: `<concrete steps; what to check in store/Redis/Kafka>`.

**2. `<symptom>`.**
Root cause: `<...>`. Mitigation: `<...>`.
