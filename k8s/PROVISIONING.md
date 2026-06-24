# Backend provisioning

The `overlays/dev` manifests deploy the **application** layer of the fleet (one
Deployment + Service + migrator init per service, see `base/services/<svc>/server`).
This doc covers the **backend** layer the services connect to.

## Dev — in-cluster (implemented)

Decision **B2 (in-cluster)**: the dev fleet runs its own backends, matching the
`overlays/dev/<svc>.env` hosts exactly. No env churn.

| backend | host | manifest |
|---------|------|----------|
| ScyllaDB (shared, 3-node RF3) | `scylla-client.scylla.svc.cluster.local:9042` | `base/infra/scylla` |
| Redis (per-service ×7)        | `dev-<svc>-redis:6379`                        | `base/infra/redis` |
| Kafka (Redpanda, 1-node)      | `dev-kafka:9092`                              | `base/infra/kafka` |
| PostgreSQL (account)          | `dev-account-postgres:5432`                   | `base/infra/postgres` |

Notes:
- **ScyllaDB is a standalone kustomization in namespace `scylla`** — its
  namespace/service names are load-bearing in the env FQDN and must NOT be
  rewritten by the dev overlay's `namePrefix: dev-`. Apply it separately:
  `kubectl apply -k k8s/base/infra/scylla`. Everything else lives in the dev
  overlay (default ns, `dev-` prefixed).
- Plain StatefulSets/Deployments (not the ScyllaDB / CloudNativePG operators):
  self-contained and fully `kubeconform`-validatable, which matters because these
  aren't applied to a live cluster from CI. 3 Scylla nodes so the migrations' RF3
  (`datacenter1:3`) places + LocalQuorum works; `scylladb/scylla:5.4` (vnodes) and
  `redpanda:v24.2.7` match the local-dev stack the migrator was smoke-tested on.
- Dev credentials are inline (they already appear in `DATABASE_URL`).

## Staging / prod — managed (future)

Decision **B1 (managed)** is the staging/prod story, not implemented yet: point
those overlays at managed Scylla Cloud / MSK / ElastiCache / RDS, with secrets and
the ScyllaDB/CNPG operators where appropriate. Tracked as a separate epic.
