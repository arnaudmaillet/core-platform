# Dev backend provisioning — known gap (tracked)

The `overlays/dev` manifests deploy the **application** layer of the fleet (one
Deployment + Service + migrator init per service, see `base/services/<svc>/server`).
They do **not** provision the **backend** layer the services connect to.

## The gap

Each service's `overlays/dev/<svc>.env` references infrastructure endpoints that
nothing in this overlay stands up:

| backend | referenced host | provisioned here? |
|---------|-----------------|-------------------|
| ScyllaDB (shared)      | `scylla-client.scylla.svc.cluster.local:9042` | ❌ |
| Redis (per-service)    | `dev-<svc>-redis:6379`                        | ❌ |
| Kafka (shared)         | `dev-kafka:9092`                              | ❌ |
| PostgreSQL (account)   | `dev-account-postgres:5432`                   | ❌ |

The only in-cluster backends currently defined are the **legacy** single-node
`base/services/profile/{postgres,redis,scylla}` (named `profile-db`, `profile-redis`,
`profile-nosql`) — a relic of the early load-test stack (#122/#123). They are kept
for now but are **not** what the new env files point at, so the dev overlay is not
end-to-end deployable until the gap below is resolved.

## Decision deferred to a dedicated epic

How dev backends are provisioned is a fleet-wide infrastructure decision, out of
scope for the deploy-templating and profile-reconciliation passes:

- **Managed/external** — point dev at managed Scylla / MSK / ElastiCache / RDS;
  retire the legacy in-cluster DBs. Env files already assume this shape.
- **In-cluster shared** — promote the legacy single-node DBs into shared in-cluster
  backends (one Scylla, Kafka, Redis tier, account Postgres) and repoint the env
  files at them. Self-contained but not production-like.

Pick one in the provisioning epic; until then, treat `overlays/dev` as the app
layer over assumed-existing infra.
