# dev → Kustomize GitOps migration

Move dev's ArgoCD off the **legacy Helm `catalog/profile-service`** path (which
deploys only the old `profile-command-server` + its own DBs) onto the **new
`k8s/overlays/dev` Kustomize fleet** (all 10 `service-runtime` services + the
in-cluster backends) — the same path staging now uses.

## Why this is gated
The Kustomize fleet **has never run on any cluster.** Migrating dev — a working
environment — onto it first makes dev the guinea pig. So:

> **Do not merge the migration PR (Phase A) until the staging Kustomize fleet has
> been applied and validated live.** The PR is held as a DRAFT; merging it puts
> `deployments/dev/fleet.yaml` on develop, and dev's ArgoCD deploys the fleet.

## Decisions (locked)
- **Sequencing:** after staging is applied + proven.
- **Cutover:** parallel-deploy, then cut (reversible at each step).
- **Profile data:** accept dev data loss (old Postgres/`profile_service` → new Scylla `profile` keyspace; no migration — it's dev).

## Phases

### A — Author (done, in the held PR; zero live effect while unmerged)
- `deployments/dev/fleet.yaml` — ArgoCD Application → `k8s/overlays/dev`, `prune: false`.
- Legacy left in place (`domain-appset.yaml`, `profile-*.values.yaml`, `catalog/profile-service/`).

### B — Parallel deploy + validate (merge the PR; live, post-staging)
1. Merge → dev ArgoCD creates `dev-fleet` and syncs `k8s/overlays/dev`.
2. The 10 services + in-cluster backends come up under `dev-` names, **without**
   disturbing the running legacy profile (distinct resources, `prune: false`).
3. Verify: migrator init-containers connect; gRPC readiness SERVING on all 10;
   backends (Scylla/Redis/Redpanda/Postgres) healthy.

### C — Cut the ingress + retire legacy
4. Repoint the public ingress (`api.dev.core-platform.click`) to `dev-profile-server`
   (the overlay ingress already targets it — consolidate onto it).
5. Delete the legacy: `domain-appset.yaml`, `profile-*.values.yaml`,
   `catalog/profile-service/`, and the chart's per-service DBs.
6. Flip `dev-fleet` `prune: false → true`.

## Risk controls
- Reversible at each step; legacy stays until Phase C; the ingress cut is a one-line revert.
- Each phase gated on the prior's health.
- ⚠️ Phases B/C are **live-cluster** actions that cannot be validated from the repo.
