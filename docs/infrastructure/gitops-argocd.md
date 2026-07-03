# GitOps & ArgoCD Operations Guide

**Document class:** Operational / Production-grade · **Audience:** DevOps &
platform engineers (application developers: see the [boundary](#what-you-own-vs-what-the-platform-owns)
section) · **Environment:** `staging` (the live GitOps path), with `dev`/`prod`
deltas inline · **Companion to:** the [infrastructure master guide](README.md).

This guide is the operational manual for the delivery plane: how manifests in Git
become running workloads, in what order, how ArgoCD keeps them there, and exactly
what to run when a sync goes wrong.

---

## 1. The delivery model in one picture

ArgoCD runs **App-of-AppSets**: a single `root-bootstrap` Application (installed by
the `kubernetes/argocd` Terragrunt unit) points at a per-environment bootstrap
folder; that folder holds ApplicationSets which fan out into the individual infra
Applications and the workload fleet.

```
 Terragrunt (kubernetes/argocd unit)
   └─ installs ArgoCD + root-bootstrap App  ──► targets bootstrap/staging/
                                                   │
        ┌──────────────────────────────────────────┼───────────────────────────────┐
        ▼                    ▼                       ▼                ▼               ▼
  root-operators      root-security          root-platform    root-observability  root-workloads
   (wave -10)          (wave -5)              (wave -5)           (wave -5)         (wave 0)
        │                    │                       │                │               │
  cnpg-operator        cert-manager            karpenter        monitoring      staging-fleet App
  external-secrets     cert-manager-config     karpenter-config                   │  (source.plugin:
  keda                 external-dns            aws-lb-controller                  │   envsubst-v1.0)
  scylla-operator      admin-access            metrics-server                     ▼
  k6-operator                                  storage-class            kustomize build k8s/overlays/staging
                                                                          | envsubst  ──► the ~17 services
```

- **Root-of-trust:** Terraform installs exactly one thing in-cluster —
  `root-bootstrap`. Everything else is Git-driven from there. This is why the
  bootstrap order (§4) has Terraform *before* any workload.
- **The tracked revision is per environment.** staging's AppSets and
  Applications set `targetRevision: develop`; **prod's set `targetRevision:
  main`** (`bootstrap/prod`, `deployments/prod`) — merging develop → main *is*
  the prod deploy, so the branch is the promotion gate. ArgoCD reconciles each
  cluster to its branch with `selfHeal: true`. **Both branches are protected —
  never push directly; branch, PR, merge, and let ArgoCD converge.**
- **Repo:** `https://github.com/arnaudmaillet/core-platform` for every source.

---

## 2. Sync waves — the load-bearing ordering

Sync waves are annotations (`argocd.argoproj.io/sync-wave`) that ArgoCD honours
lowest-first. The ordering is **not cosmetic**: an operator's CRDs must be
established before any workload applies a custom resource that references them, or
the workload sync fails with `no matches for kind`.

| Wave | AppSet | Contents | Why it must go first |
|---:|---|---|---|
| **−10** | `root-operators` | CNPG operator, External Secrets, **KEDA**, scylla-operator, k6-operator | Install the CRDs (`Cluster`, `ExternalSecret`, `ScaledObject`, `ScyllaCluster`) the fleet depends on. |
| **−5** | `root-security` | cert-manager (+config), external-dns, admin-access | TLS issuers and DNS must exist before Ingress/Service objects request certs/records. |
| **−5** | `root-platform` | Karpenter (+config), aws-lb-controller, metrics-server, storage-class | Compute autoscaling, the LB controller (provisions NLB/ALB), the **gp3 StorageClass**, and HPA's metrics source. |
| **−5** | `root-observability` | monitoring | Scrape targets and dashboards. |
| **0** | `root-workloads` | `staging-fleet` Application → `k8s/overlays/staging` | The services themselves — they declare `ScaledObject`/`Cluster`/`ExternalSecret` CRs, so they must be last. |

> **Failure mode — CRD race.** If you see a workload App stuck `SyncFailed` with
> `unable to recognize "...": no matches for kind "ScaledObject"`, an operator at
> wave −10 has not finished. Do **not** retry the workload; fix the operator first
> (§7.2), then the workload sync succeeds unchanged.

### Prune policy differs by tier, on purpose

- **Infra AppSets** (`root-operators`, etc.) run `prune: true` — the platform is
  fully declarative; anything not in Git should be removed.
- **`root-workloads` (dev legacy)** runs `prune: false` — a guard against an
  accidental mass-delete of running services from a bad generator.
- **`staging-fleet`** runs `prune: true` + `selfHeal: true` — staging is the
  managed, disposable fleet and must match Git exactly.

---

## 3. The envsubst Config Management Plugin (why `source.plugin`, not `path`)

The staging overlay carries **runtime endpoints that are only known after the
Terraform data-store apply** — MSK brokers, the ElastiCache/OpenSearch endpoints,
the ACM certificate ARN. These live in the manifests as `${VAR}` placeholders.

They are resolved by an **`envsubst` Config Management Plugin** running as a sidecar
in `argocd-repo-server` (defined in `modules/kubernetes/argocd`). The plugin runs:

```
kustomize build k8s/overlays/staging | envsubst
```

over a **Terraform-owned values Secret** (`cmp-envsubst-values`). The
`kubernetes/argocd` Terragrunt unit writes that Secret from data-store outputs
(`msk_bootstrap_brokers`, `elasticache_endpoint`, `opensearch_endpoint`,
`ssl_certificate_arn`). Hence the `staging-fleet` Application references
`source.plugin.name: envsubst-v1.0`, **not** a bare `path`.

```
Terraform data-store outputs ──► kubernetes/argocd unit ──► cmp-envsubst-values Secret
                                                                     │
  Git: k8s/overlays/staging (${VAR} templates) ── repo-server CMP ───┘
        │
        └─ kustomize build | envsubst ──► concrete manifests ──► cluster
```

**Why this design and not committed concrete values:** Git holds the *template*;
the CMP renders concrete manifests at sync time. Because the rendered output is
deterministic from Git + the Secret, **`selfHeal` is stable** — there are no manual
in-cluster edits for ArgoCD to fight. The plugin is named
`<metadata.name>-<spec.version>`, so the plugin defined as `envsubst` / `v1.0` is
referenced as **`envsubst-v1.0`**.

> **Failure mode — unresolved placeholder.** A pod env showing a literal
> `${MSK_BOOTSTRAP_BROKERS_SASL_SCRAM}` means the CMP ran but the Secret lacked the
> key. The value is Terraform-owned: re-run the `kubernetes/argocd` unit so it
> re-writes `cmp-envsubst-values`, then hard-refresh the App (§6). Placeholders that
> stay empty on purpose until a dependency lands: `AUTH_JWKS_URL`,
> `KEYCLOAK_TOKEN_ENDPOINT` (DEFERRED until Keycloak).

---

## 4. Bootstrap order — Terraform, then GitOps converges

The delivery plane cannot start before the platform it runs on exists. The full
sequence (data-store detail in the [Terragrunt units reference](terragrunt-units.md)):

```
1. Terragrunt run-all apply           # vpc → eks → data/* → security/irsa-roles → kubernetes/argocd
                                       #   (the argocd unit installs ArgoCD + root-bootstrap
                                       #    and writes cmp-envsubst-values + global-params-staging.json)
2. GitOps: operators converge         # wave -10 — CNPG, scylla-operator, ESO, KEDA come Healthy
3. kubectl apply -k k8s/base/infra/scylla-cluster   # the ScyllaCluster CR (un-prefixed FQDN)
4. GitOps: security/platform/observability   # wave -5
5. GitOps: staging-fleet syncs         # wave 0 — the services
```

Watch it converge:

```bash
# after the Terragrunt apply completes
aws eks update-kubeconfig --name <cluster> --region us-east-1
kubectl -n argocd get applications -w        # wait for root-* then staging-fleet to be Synced/Healthy
```

The authoritative, annotated end-to-end sequence lives in
[`runbooks/audit-remediation-rollout.md`](../runbooks/audit-remediation-rollout.md);
the full disposable loop (including teardown) is in
[`runbooks/environment-lifecycle.md`](../runbooks/environment-lifecycle.md).

---

## 5. The infra app catalog (what each App is)

`infrastructure/argocd/apps/infrastructure/` — grouped by the AppSet that fans them
out. Each is a thin Helm/Kustomize App layered with `global-params-staging.json`.

| Group (wave) | App | Role |
|---|---|---|
| **operators (−10)** | `cnpg-operator` | CloudNativePG — the 6 in-cluster Postgres clusters. |
| | `external-secrets` | ESO — projects AWS Secrets Manager into k8s Secrets (see [secret topology](secrets-eso.md)). |
| | `keda` | Kafka-lag autoscaling for stream workers. |
| | `scylla-operator` | Manages the ScyllaCluster. |
| | `k6-operator` | Load-test orchestration. |
| **security (−5)** | `cert-manager` / `-config` | TLS issuance; config wires the issuer(s). |
| | `external-dns` | Route53 records for Services/Ingress. |
| | `admin-access` | RBAC / admin bindings. |
| **platform (−5)** | `karpenter` / `-config` | Node autoscaling controller + NodePools/EC2NodeClasses. |
| | `aws-lb-controller` | Provisions the NLB (realtime WSS) / ALBs. |
| | `metrics-server` | Feeds HPA. |
| | `storage-class` | The **gp3** default StorageClass (the storage plane). |
| **observability (−5)** | `monitoring` | Metrics/dashboards. |

Every App receives `global-params-staging.json` (account ID, region, cluster name,
addon IAM role ARNs) via a Helm `$values` ref or the params file, so the same App
definitions render per-environment without duplication.

---

## 6. Day-2 operations (exact invocations)

All commands assume `kubectl` is pointed at the target cluster and ArgoCD is in the
`argocd` namespace. Prefer the ArgoCD API/UI for sync; `kubectl` is the escape hatch.

```bash
# Inventory & health
kubectl -n argocd get applications                      # every App, sync + health status
kubectl -n argocd get applicationsets                   # the root-* generators
argocd app list                                         # same via CLI (after `argocd login`)

# Inspect one App
argocd app get staging-fleet
kubectl -n argocd get application staging-fleet -o yaml | yq '.status.conditions'

# Force a re-read of Git (after a merge to develop that Argo hasn't picked up)
argocd app get staging-fleet --refresh                  # soft: re-read Git
argocd app get staging-fleet --hard-refresh             # hard: also re-run the CMP (envsubst)

# Manually trigger a sync (normally automated)
argocd app sync staging-fleet
argocd app sync staging-fleet --prune                   # allow deletes (staging only)

# See exactly what a sync would change
argocd app diff staging-fleet

# Roll back to the previous synced revision
argocd app history staging-fleet
argocd app rollback staging-fleet <history-id>
```

### Pausing reconciliation (maintenance / incident)

`selfHeal` will revert manual `kubectl edit`s within seconds. To make a deliberate
out-of-band change stick, **suspend** the App first:

```bash
# Pause auto-sync so a manual change survives
kubectl -n argocd patch application staging-fleet --type merge \
  -p '{"spec":{"syncPolicy":{"automated":null}}}'
# ... do the manual thing ...
# Re-enable (let Git win again)
kubectl -n argocd patch application staging-fleet --type merge \
  -p '{"spec":{"syncPolicy":{"automated":{"prune":true,"selfHeal":true}}}}'
```

> The teardown hook uses the same lever at the root:
> `kubectl patch app root-bootstrap -n argocd --type merge -p '{"spec":{"syncPolicy":null}}'`
> to stop ArgoCD recreating what a destroy deletes.

---

## 7. Failure modes & recovery

### 7.1 App stuck `OutOfSync` / `Progressing` forever

```bash
argocd app get <app>                       # read the message on each resource
kubectl -n argocd logs deploy/argocd-application-controller | tail -50
```

- **Immutable-field conflict** (e.g. a Deployment `selector` change): ArgoCD can't
  patch it. Delete the offending resource and let the sync recreate it
  (`argocd app sync <app> --resource <group:kind:name> --force`).
- **ServerSideApply field manager conflict:** all Apps use `ServerSideApply=true`;
  a conflicting manager needs `--force` on the sync.

### 7.2 Workload sync fails with `no matches for kind "ScaledObject"/"Cluster"`

The operator (wave −10) has not established its CRD. Check and heal the operator,
not the workload:

```bash
kubectl -n argocd get application keda cnpg-operator scylla-operator external-secrets
kubectl get crd | grep -E 'scaledobjects|clusters.postgresql|scyllaclusters|externalsecrets'
argocd app sync keda                       # re-drive the operator, then the workload converges on its own
```

### 7.3 `selfHeal` fighting a manual change (change keeps reverting)

Expected behaviour — Git is the source of truth. Either commit the change to
`develop` (correct path) or suspend automation (§6) for a deliberate temporary
override. Never disable `selfHeal` in Git to win an argument with it.

### 7.4 CMP / envsubst render failures (`staging-fleet` only)

```bash
kubectl -n argocd logs deploy/argocd-repo-server -c envsubst | tail -50   # the sidecar
kubectl -n argocd get secret cmp-envsubst-values -o yaml                   # the Terraform-owned values
```

- Missing/empty key → re-run the `kubernetes/argocd` Terragrunt unit (it owns the
  Secret), then `argocd app get staging-fleet --hard-refresh`.
- A `kustomize build` error → validate locally first: `kubectl kustomize k8s/overlays/staging`.

### 7.5 CRD-reference transformer gaps (prefix silently wrong)

Kustomize's built-in `nameReference` transformer does **not** know KEDA
`ScaledObject.scaleTargetRef` or CNPG `ScheduledBackup.spec.cluster.name`. The
overlays add `configurations:` entries (`*-refs-config.yaml`) so `namePrefix`
flows. Symptom of a missing one: a prefixed scaler/backup that targets a
non-existent (unprefixed) resource — the App is `Healthy` but nothing scales/backs
up. When you introduce a CRD that references another resource by name, add the
config entry (see the master guide §1.4 and the Conventions in `CLAUDE.md`).

---

## 8. What you own vs what the platform owns

**Application developers — you interact with GitOps at exactly three seams; you do
not operate ArgoCD:**

1. **Your manifests** live in `k8s/base/services/<svc>` and are layered by the
   overlays. Merging to `develop` is your deploy trigger — ArgoCD does the rest.
2. **Your image tag.** The staging overlay is pinned to immutable `:<git-sha>` tags
   by the fleet CI job. Don't reintroduce a floating `:staging` tag — ArgoCD won't
   redeploy on a mutable-tag re-push without a digest bump.
3. **Your config/secrets** arrive as env from `${VAR}` placeholders (endpoints) and
   mounted Secrets (credentials). To add one, follow the
   [secret topology guide](secrets-eso.md) — you do not touch ArgoCD.

**Platform engineers own:** the AppSets, sync waves, the CMP, the infra app
catalog, and the `develop` protection. Everything in §§1–7 is yours.

---

## Appendix — quick reference

```bash
# Where things live
infrastructure/argocd/bootstrap/                 # root AppSets (root-infra-*, root-appset-workloads)
infrastructure/argocd/bootstrap/staging/         # staging's per-env bootstrap (root-bootstrap targets this)
infrastructure/argocd/apps/infrastructure/       # the infra app catalog (operators/security/platform/observability)
infrastructure/argocd/apps/deployments/staging/  # staging-fleet Application (source.plugin: envsubst-v1.0)
infrastructure/argocd/bootstrap/global-params*   # per-env params layered onto every App
k8s/overlays/staging/                            # the fleet the CMP renders

# One-liners
kubectl -n argocd get applications                              # fleet-wide status
argocd app sync staging-fleet && argocd app wait staging-fleet  # sync + block until Healthy
argocd app get staging-fleet --hard-refresh                     # re-render the CMP
```
