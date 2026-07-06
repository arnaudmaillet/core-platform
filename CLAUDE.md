# CLAUDE.md

Guidance for working in `core-platform`. Keep this file short and factual — it is
loaded into every session. Deep detail lives in `docs/` (linked below).

## What this is

A single Rust workspace (`crates/`) for a hyperscale, event-driven social backend:
~17 services as hexagonal (DDD) crates, each shipped as one or more per-binary
container images. Sync contracts are versioned gRPC (`*-api` crates); async
contracts are Kafka topics governed by the `event-topology` registry. Infra is
Terraform/Terragrunt + EKS + Karpenter + ArgoCD (GitOps via Kustomize).

> `staging` is the active GitOps path (`k8s/overlays/staging`, synced by ArgoCD
> from `develop`). **prod is fully scaffolded but NOT applied**: `live/prod` +
> `k8s/overlays/prod` + `bootstrap/prod` track the `main` branch (merging
> develop → main is the prod deploy); bring-up prerequisites live in
> `infrastructure/live/prod/env.hcl` (state bucket, EKS endpoint CIDRs,
> image-promotion workflow).

## Repo layout

| Path | What |
|---|---|
| `crates/foundation`, `crates/platform`, `crates/storage` | shared libs (error, health, resilience, cqrs, transport, telemetry, auth-context, Postgres/Scylla/Redis adapters, `service-runtime`, `test-support`) |
| `crates/services/<svc>` | per-service hexagonal crate (`domain → application(ports) → infrastructure(adapters)`) |
| `crates/apps/<svc>-server` (and `-worker`) | thin deployable binaries (`serve::<XService>(addr)`) |
| `crates/contracts/<svc>-api`, `crates/contracts/proto` | gRPC contract crates + protos |
| `crates/contracts/event-topology` | **authoritative Kafka producer/consumer registry** (golden-tested; generates `docs/domain/EVENT_CATALOG.md`) |
| `k8s/base` + `k8s/overlays/{dev,staging,prod}` | Kustomize manifests (staging is the live fleet) |
| `infrastructure/modules` + `infrastructure/live/<env>` | Terraform modules + Terragrunt live tree |
| `infrastructure/argocd` | ArgoCD bootstrap appsets + Helm app catalog |
| `docs/` | architecture (ADRs), domain (event catalog, context map), infrastructure, runbooks, security |

## Common commands

```bash
# Build / test / lint the workspace
cargo build --workspace
cargo clippy --workspace --all-targets
cargo test  --workspace                 # unit tests; integration tests are feature-gated:
cargo test -p <svc> --features integration-<svc>   # needs Docker (Scylla/Redis/Kafka/PG containers)

# Contracts
( cd crates/contracts/proto && buf lint )          # buf breaking runs in CI on PRs;
                                                   # merges to develop/main publish to
                                                   # buf.build/core-platform/contracts (needs BUF_TOKEN)

# Build a service image (one image per binary)
docker build -f deploy/Dockerfile --build-arg BIN=<svc>-server -t <repo>/<svc>-server:<tag> .

# Render manifests (validate before pushing)
kubectl kustomize k8s/overlays/staging

# i18n drift gate (MUST pass — see i18n rule below)
bash tools/i18n/i18n-drift.sh check
bash tools/i18n/i18n-drift.sh stamp <file>.fr.md   # re-stamp after editing an EN source

# Infra (per env, from infrastructure/live/<env>/us-east-1)
terragrunt run-all plan
```

## Service & gRPC port registry

`chat` 50051 · `profile` 50052 · `social-graph` 50053 · `geo-discovery` 50054 ·
`notification` 50055 · `post` 50056 · `comment` 50057 · `engagement` 50058 ·
`account` 50059 · `auth` 50060 · `moderation` 50061 · `search` 50062 · `media` 50063 ·
`counter-server` 50064 · `counter-worker` 50065 · `realtime-gateway` 50066 (gRPC) + **8443** (public WSS) ·
`realtime-dispatcher` 50067 · `audit-server` 50068 · `audit-worker` 50069 · `timeline` **50070**

One port per service. (`timeline` was 50060, moved to 50070 to clear a collision
with `auth` — PR #522.) Each service owns an error-code namespace, e.g. `TML-`
(timeline), `SCH-` (search), `MED-` (media), `CTR-` (counter), `AUD-` (audit),
`RTM-` (realtime), `SGR-`, `PST-`, etc.

## Conventions (do these)

- **Service shape:** hexagonal — keep domain pure, put I/O behind `application/port`
  traits, implement in `infrastructure`. New binaries are thin: read addr from
  `<SVC>_GRPC_ADDR`, call `service_runtime::serve::<XService>(addr)`.
- **Kafka consumers MUST use `run_consumer`** (manual commit after success/DLQ,
  backoff+jitter, domain idempotency). Never hand-roll a consume loop.
- **Async contracts go through the `event-topology` registry** — adding a
  producer/consumer edge means editing the registry (a contract test fails on a
  "phantom edge"); then regenerate the catalog (`tools/event-catalog/sync.sh`).
  The registry is also what provisions the brokers: the `topic-provisioner`
  PreSync Job creates every stream topic + `.dlq` (MSK runs with topic
  auto-creation off) — a registry edit is the whole workflow.
- **Service tiers** are an explicit runtime contract (pod label `tier:`): TIER-0 =
  fail-closed (`auth`, `moderation`, `audit`); TIER-1 = fail-open
  (`counter`, `media`, `search`, `realtime`). Respect the posture when adding code.
- **Kustomize CRD references:** the built-in nameReference transformer doesn't know
  KEDA `ScaledObject.scaleTargetRef` or CNPG `ScheduledBackup.spec.cluster.name` —
  overlays add `configurations:` entries (`*-refs-config.yaml`) so `namePrefix`
  flows. Add one when introducing a CRD that references another resource by name.

## GitOps / IaC rules

- **ArgoCD tracks `develop` with `selfHeal`.** `develop` is **protected** — branch
  off it, open a PR; don't commit/push to it directly.
- **Apply order matters.** Terraform must run before the workloads sync: the
  staging overlay's runtime endpoints are resolved by an `envsubst` Config
  Management Plugin in `argocd-repo-server` (fed by a Terraform-written Secret), and
  CNPG backups / Karpenter graceful-drain / NetworkPolicies all depend on AWS
  resources Terraform creates. Order: `vpc → eks → data → security → argocd`, then
  let ArgoCD sync. Full sequence: **`docs/runbooks/audit-remediation-rollout.md`**.
- **Image tags:** the staging overlay is pinned to immutable `:<git-sha>` tags by
  the fleet CI job (not a mutable `:staging`). Don't reintroduce floating tags.

## i18n rule

English is canonical; French is a co-located `*.fr.md` whose YAML frontmatter
records the SHA-256 of the EN source it was translated from. **If you edit an EN
doc that has a `.fr.md`, update the FR too and re-stamp** (`i18n-drift.sh stamp`),
or CI fails. Contracts (error codes, env vars, topic names, identifiers) stay in
English inside FR files. See `docs/i18n/`.

## Key references

- Infra & ops overview (canonical): `docs/infrastructure/README.md`
- Docs entry point & taxonomy: `docs/README.md`
- GitOps / ArgoCD operations: `docs/infrastructure/gitops-argocd.md`
- Terragrunt units reference: `docs/infrastructure/terragrunt-units.md`
- Secret topology (ESO / ClusterSecretStore): `docs/infrastructure/secrets-eso.md`
- Environment lifecycle runbook: `docs/runbooks/environment-lifecycle.md`
- Event plane (who produces/consumes what): `docs/domain/EVENT_CATALOG.md`
- Domain context map / ubiquitous language: `docs/domain/`
- Architecture decisions: `docs/adr/`
- Rollout runbook: `docs/runbooks/audit-remediation-rollout.md`
- NetworkPolicy call graph (W8): `docs/security/network-policy-call-graph.md`
