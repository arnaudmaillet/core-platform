# Runbook: Environment Lifecycle (preflight → provision → validate → teardown → rebuild)

**Document class:** Runbook / Production-grade · **Audience:** DevOps engineers ·
**Environment:** `staging` (the disposable, live GitOps path) · **Companion to:**
the [GitOps guide](../infrastructure/gitops-argocd.md), the
[Terragrunt units reference](../infrastructure/terragrunt-units.md), and the
[disposable staging rebuild](staging-disposable-rebuild.md) runbook.

Staging is a **disposable environment**: it is stood up from scratch, validated,
and torn down repeatedly. This runbook is the end-to-end loop and the ordering
constraints that keep each phase safe. The two narrow, well-worn hazards —
Secrets-Manager/KMS deletion state and load-balancer/ENI leaks — have dedicated
tooling that this runbook drives at the right moment.

```
   ┌──────────┐   ┌───────────┐   ┌──────────┐   ┌────────────────────┐
   │ PREFLIGHT│──►│ PROVISION │──►│ VALIDATE │──►│ GRACEFUL TEARDOWN  │──┐
   └──────────┘   └───────────┘   └──────────┘   └────────────────────┘  │
        ▲                                                                 │
        └─────────────────────  REBUILD  ────────────────────────────────┘
```

Set this once per session:

```bash
BASE=infrastructure/live/staging/us-east-1
export AWS_REGION=us-east-1
```

---

## Phase 1 — Preflight (before every apply)

A rebuild started too soon after a teardown collides with AWS state that outlives
`terragrunt destroy`. **Always preflight**, even on a "clean" account.

```bash
# Report-only: names still reserved by a prior teardown, orphan KMS keys
bash infrastructure/assets/teardown/preflight-clean-env.sh staging

# If anything is RESERVED, clear it (restore + force-delete to free the name)
bash infrastructure/assets/teardown/preflight-clean-env.sh staging --fix
```

**What it checks and why (learned the hard way):**

- A Secrets Manager secret in `PendingDeletion` **reserves its name for the whole
  recovery window**, and `describe-secret` returns `NotFound` for it — so a naive
  "does it exist?" check reports the name as free when it is not. The script uses
  `list-secrets --include-planned-deletion` to see the truth.
- Modules set `recovery_window_in_days = 0` (PR #538) so teardowns delete secrets
  immediately — but AWS still **reserves a force-deleted name for a few minutes**.
- **KMS** keys have a 7-day minimum deletion window (no force-immediate); a rebuild
  makes fresh keys, so pending keys are orphan cost, not a blocker — *unless* you
  import a stale secret whose key is pending (`KMSInvalidStateException`).

> **Cooldown:** if `--fix` cleared anything, **wait ~15 minutes** before Phase 2, or
> `CreateSecret` will still collide on the just-freed name. This is the single most
> common rebuild failure — respect it.

The deep dive on the deletion-state gotchas and the import/adopt alternative lives
in [staging-disposable-rebuild.md](staging-disposable-rebuild.md).

---

## Phase 2 — Provision (Terraform, then GitOps converges)

Terraform stands up the platform; ArgoCD then converges the fleet. The apply DAG
and per-unit detail are in the
[Terragrunt units reference](../infrastructure/terragrunt-units.md); the full
provisioning checklist (endpoint placeholders, ScyllaCluster, secret seeding) is in
[`k8s/PROVISIONING-staging.md`](../../k8s/PROVISIONING-staging.md). The loop-level
sequence:

```bash
# 1. Terraform: whole tree, in dependency order (vpc → eks → data/* →
#    security/irsa-roles → kubernetes/argocd). GITHUB_TOKEN is required —
#    the argocd unit registers the repo with ArgoCD.
( cd $BASE && GITHUB_TOKEN=$(gh auth token) \
    terragrunt run --all apply --non-interactive --backend-bootstrap -- -auto-approve )

# 2. Point kubectl at the fresh cluster
aws eks update-kubeconfig --name <cluster> --region "$AWS_REGION"

# 3. Watch GitOps converge: operators (wave -10) → security/platform (wave -5) → fleet (wave 0)
kubectl -n argocd get applications -w

# 4. Apply the ScyllaCluster CR once scylla-operator is Healthy (un-prefixed FQDN)
kubectl apply -k k8s/base/infra/scylla-cluster
```

**Ordering constraints that must hold (each is a real failure mode):**

- `security/irsa-roles` applies **after** the data stores — it consumes their ARNs
  (audit KMS/WORM, media/cnpg buckets). `mock_outputs` allow the earlier `plan`.
- Operators at **wave −10** must be Healthy before the workload fleet (wave 0)
  applies its `ScaledObject`/`Cluster`/`ExternalSecret` CRs — otherwise
  `no matches for kind`. See [GitOps §2](../infrastructure/gitops-argocd.md#2-sync-waves--the-load-bearing-ordering).
- The `kubernetes/argocd` unit writes the **`cmp-envsubst-values`** Secret; without
  it the fleet renders literal `${VAR}` endpoints. Re-run that unit if placeholders
  don't resolve.

> **Trust the Run Summary (`Succeeded / Failed`), NOT the exit code** — `terragrunt
> run --all` can exit `0` with failed units.

---

## Phase 3 — Validate

Confirm the platform is actually serving before declaring the environment up.

```bash
# Terraform side — zero failed units, endpoints resolvable
( cd $BASE && terragrunt run-all output 2>/dev/null | grep -E 'endpoint|brokers|arn' )

# GitOps side — every App Synced + Healthy
kubectl -n argocd get applications           # no OutOfSync / Degraded
argocd app get staging-fleet                 # the workload App specifically

# Secrets materialized (ESO did its job)
kubectl get externalsecret -A                # all SecretSynced=True
kubectl get secret backend-creds -o jsonpath='{.data}' | jq 'keys'

# Compute & storage plane
kubectl get nodes -l karpenter.sh/nodepool   # Karpenter provisioned nodes
kubectl get storageclass                     # gp3 is default
kubectl get pods -A | grep -vE 'Running|Completed'   # nothing stuck

# Stateful backends
kubectl get clusters.postgresql.cnpg.io -A   # 6 CNPG clusters Healthy
kubectl get scyllaclusters.scylla.scylladb.com -A
```

**Expected known-degraded (not failures):**

- `realtime` WSS plane fails closed (`RTM-1001`) until `auth`'s JWKS is reachable —
  Keycloak is **DEFERRED**, so this is expected. The gRPC health plane is
  unaffected, so the pod still becomes Ready.
- `AUTH_KEYCLOAK_CLIENT_SECRET` is a placeholder until Keycloak lands.

If a placeholder endpoint leaked into a pod (`${MSK_BOOTSTRAP_BROKERS_SASL_SCRAM}`
literal), fix per the [GitOps CMP failure mode](../infrastructure/gitops-argocd.md#34-cmp--envsubst-render-failures-staging-fleet-only).

---

## Phase 4 — Graceful teardown

**Never `terraform destroy` a live cluster blind.** In-cluster controllers
(AWS LB controller, CNPG, scylla-operator, Karpenter) create AWS resources that
Terraform does **not** own; a blind destroy leaks them, and leftover
load-balancer ENIs block `aws_vpc` destroy with `DependencyViolation`.

The `kubernetes/argocd` unit carries a **`before_hook "graceful_cleanup"` on
`destroy`** that runs `infrastructure/assets/teardown/k8s-graceful-cleanup.sh`
automatically — you just run the normal destroy:

```bash
( cd $BASE && terragrunt run --all destroy --non-interactive -- -auto-approve )
```

Because `destroy` walks the DAG in reverse, `kubernetes/argocd` tears down first,
firing the hook **before** `eks`/`vpc` are touched. The hook, in order:

1. **Stops ArgoCD self-heal** (`patch app root-bootstrap … syncPolicy:null`; delete
   appsets) so it can't recreate what's being deleted.
2. **Deletes Ingresses (ALBs) and `type=LoadBalancer` Services (NLBs)**, then
   **waits (~5 min) for AWS to actually deprovision them** — `kubectl delete svc`
   returns before the LB controller has removed the real NLB/ENIs. This wait is what
   prevents the ACM-cert `ResourceInUseException` race and the VPC ENI leak.
3. **Deletes CNPG/Scylla CRs then PVCs**, so `ebs-csi` issues `DeleteVolume` (the
   `reclaimPolicy=Delete` only fires on orderly PVC deletion).
4. **Deletes remaining ArgoCD apps except Karpenter.**
5. **Drains Karpenter NodeClaims/nodes while Karpenter still runs**, so it
   terminates the EC2 instances (and their ENIs/EBS) via the EC2 API.

Every step is best-effort (`|| true`) and idempotent — a partially-broken cluster
must never block the destroy.

### If teardown leaves stale state (the ACM race)

If `eks` fails deleting its ACM cert (`ResourceInUseException`) and `vpc`
early-exits, the AWS resources are usually gone but the unit *state* is stale.
Reconcile per-unit, then confirm zero leaks:

```bash
( cd $BASE/networking/acm-cert && terragrunt state list )   # inspect first
( cd $BASE/networking/vpc && terragrunt state rm $(cd $BASE/networking/vpc && terragrunt state list) )
aws ec2 describe-vpcs --filters Name=isDefault,Values=false  # expect none (ignore list-flicker; verify by --vpc-ids)
```

The decoupled `acm-cert` unit (PR #543) and the LB-deprovision wait make this rare.

---

## Phase 5 — Rebuild

A rebuild is just **Phase 1 → Phase 2 → Phase 3** again. The only rebuild-specific
concern is the deletion-state debt Phase 1 clears. If a data-store unit fails on
`already scheduled for deletion` during Phase 2, a name is still reserved:

- **Clear (preferred):** `preflight-clean-env.sh staging --fix`, wait ~15 min, re-apply.
- **Adopt (no wait, MSK-risky):** restore + `terragrunt import` the secret — but MSK
  hits `KMSInvalidStateException` if the key is pending; **prefer Clear for MSK**.
  Full procedure in [staging-disposable-rebuild.md](staging-disposable-rebuild.md).

---

## Quick reference — the whole loop

```bash
BASE=infrastructure/live/staging/us-east-1 ; export AWS_REGION=us-east-1

# PREFLIGHT
bash infrastructure/assets/teardown/preflight-clean-env.sh staging --fix   # wait ~15m if it cleared anything

# PROVISION
( cd $BASE && GITHUB_TOKEN=$(gh auth token) \
    terragrunt run --all apply --non-interactive --backend-bootstrap -- -auto-approve )
aws eks update-kubeconfig --name <cluster> --region "$AWS_REGION"
kubectl apply -k k8s/base/infra/scylla-cluster

# VALIDATE
kubectl -n argocd get applications ; kubectl get externalsecret -A ; kubectl get nodes -l karpenter.sh/nodepool

# TEARDOWN (graceful hook fires automatically)
( cd $BASE && terragrunt run --all destroy --non-interactive -- -auto-approve )
```

---

## Boundary note

Everything in this runbook is **platform-layer** — provisioning, GitOps
convergence, and cloud teardown. Application developers never run these commands; a
service ships by merging to `develop` and letting ArgoCD sync (see the
[GitOps boundary section](../infrastructure/gitops-argocd.md#8-what-you-own-vs-what-the-platform-owns)).
