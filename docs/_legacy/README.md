# ⚠️ Legacy documentation — DO NOT TRUST

> **Status: QUARANTINED.** Everything in this directory describes a **pre-fleet design**
> that diverged from — or was never built as — the system that exists today. It is kept
> only for historical reference and is **excluded from the source of truth**.
>
> For the current architecture, see:
> - **What exists (ground truth):** the crates themselves — `crates/services/*`, `crates/apps/*`,
>   their `*.v1` proto contracts, and the event-topology registry guard.
> - **How it deploys & operates:** [`docs/infrastructure/`](../infrastructure/README.md).
> - **What each service does (domain/functional):** [`docs/domain/`](../domain/README.md)
>   and each service's `crates/services/<svc>/docs/DOMAIN.md`.

## Why this was quarantined

The C4 model here (`workspace.dsl`, `deployment.dsl`, `architecture/`, `flows/`, `styles/`,
`icons/`, `workspace.json`, `.structurizr/`) was authored before the current 17-service fleet
existed and was never reconciled with it. Concretely, it is wrong in ways that actively mislead:

| The legacy model says | Ground truth |
|---|---|
| `API BFF`, `Real-time BFF`, `Redis (BFF)` | No BFF crate exists; the live edge is the `realtime-gateway` / `realtime-dispatcher` pair |
| `Recommendation Service` | No such service |
| `Feed Service` | The real service is **`timeline`** |
| `Analytics Collector` + `Analytics Worker` + `ClickHouse` + `Data Lake` | The real service is **`counter`** (counter-analytics); cold tier is **Scylla TWCS** |
| `NebulaGraph` | `social-graph` uses **ScyllaDB + Redis** |
| `Elasticsearch` | `search` uses **OpenSearch** |
| `Rust/Axum` (≈15 containers) | Services are **gRPC/tonic on `service-runtime`**; only `realtime-gateway` uses axum (WebSockets) |
| `Comment Service` declared twice | duplication bug |
| — | Entirely **missing**: `audit` (TIER-0), `auth` (TIER-0), `chat`, `geo-discovery`, `media`, `realtime`, `counter` |

Provenance: authored in commits #320–#322 (*"BFF aggregation,"* *"hybrid push/pull"*),
before the current fleet, and never updated since.

## Disposition

This C4 model has been **superseded by a corrected model regenerated from the functional
documentation** (the per-service Domain Cards plus `docs/domain/CONTEXT_MAP.md`), now living at
[`docs/architecture/`](../architecture/README.md). This `_legacy/` directory is retained only for
historical reference and **may be deleted outright** — nothing current references it.
