# Rollout Runbook — Infra/SecOps Audit Remediation

Ordered apply/rollout for the 12 audit-remediation PRs now merged to `develop`
(#508–#519). Written 2026-06-29.

> **Why order matters.** ArgoCD tracks `develop` with `selfHeal`, so once a cluster
> is bootstrapped it syncs the workload overlay automatically. Several of those
> manifests now depend on AWS resources that **Terraform must create first** — the
> repo-server `envsubst` CMP needs a values Secret + data-store endpoints (#509),
> CNPG backups need a bucket + IRSA role (#512), Karpenter graceful-drain needs an
> SQS queue (#513), and the NetworkPolicies need CNI enforcement turned on (#519).
> Apply Terraform **before** letting the workloads sync, or those render empty / error.

Target: **staging** (`core-platform-staging`, us-east-1, account `724772065879`).
The account is idle, so this is a clean stand-up. Run each phase, validate, then proceed.

---

## Phase 0 — Pre-flight

- [ ] Confirm you can reach the EKS API and have admin (`kubectl auth can-i '*' '*'`).
- [ ] Confirm Terraform/Terragrunt creds (the same identity the GitHub provider uses
      for the global-params write-back — it must be able to push to `develop`).
- [ ] `git -C infrastructure/live/staging pull` so the working tree is at the merged `develop`.
- [ ] Note: image tags in `k8s/overlays/staging/kustomization.yaml` are still
      `:staging` on `develop`. The C4 pin job rewrites them to immutable `:<git-sha>`
      on the **next** fleet build (push to `develop` touching `crates/**`). First sync
      therefore runs `:staging`; that's expected.

---

## Phase 1 — Terraform / Terragrunt (provision the infra the GitOps layer needs)

From `infrastructure/live/staging/us-east-1`. Terragrunt resolves the dependency
graph, but the critical path is: **vpc → eks → data stores → security → argocd**.

### 1a. Plan everything first
```bash
cd infrastructure/live/staging/us-east-1
terragrunt run-all plan 2>&1 | tee /tmp/staging-plan.txt
```
Review for surprises. Expected new/changed resources by area:

| Unit | What changed | Audit ref |
|---|---|---|
| `networking/vpc` | NAT honors `single_nat_gateway` (staging stays 1); **S3 Gateway endpoint** (free) on private+data RTs; interface-endpoint scaffolding (off by default) | W1/W2 |
| `eks` | `cluster_endpoint_private_access=true`; `public_access_cidrs` knob (still `0.0.0.0/0`); node SG 8080 now uses real VPC CIDR; **vpc-cni `enableNetworkPolicy=true`** | W3/W4/W8 |
| `data/cnpg-backups` | **new** versioned SSE bucket for CNPG backups | C6 |
| `security/irsa-roles` | **Karpenter interruption SQS queue + 4 EventBridge rules**; **cnpg-backup IRSA role** | W5/C6 |
| `kubernetes/argocd` | repo-server **envsubst CMP sidecar** + `cmp-envsubst-values` Secret (MSK/Redis/OpenSearch/ACM); now depends on the data stores | C1 |

> ⚠️ **`enableNetworkPolicy` on vpc-cni is an addon update** — it rolls the aws-node
> DaemonSet. Brief, but it is a CNI change. Do it in a maintenance window if the
> cluster is live. (On a fresh cluster: harmless.)

### 1b. Apply in order
```bash
terragrunt run-all apply
# If you prefer manual control, apply per unit in this order:
#   networking/vpc → eks → data/* → security/irsa-roles → kubernetes/argocd
```

### 1c. Validate Phase 1
```bash
# NAT / S3 endpoint
aws ec2 describe-vpc-endpoints --filters Name=vpc-id,Values=<vpc> \
  --query 'VpcEndpoints[].{svc:ServiceName,type:VpcEndpointType}'

# EKS: private access on, CNI network policy on
aws eks describe-cluster --name core-platform-staging \
  --query 'cluster.resourcesVpcConfig.{pub:endpointPublicAccess,priv:endpointPrivateAccess,cidrs:publicAccessCidrs}'
kubectl -n kube-system get ds aws-node -o yaml | grep -i ENABLE_NETWORK_POLICY   # expect "true"

# Karpenter interruption queue exists (name == cluster name)
aws sqs get-queue-url --queue-name core-platform-staging

# CNPG backup role + bucket
aws iam get-role --role-name core-platform-staging-cnpg-backup-role >/dev/null && echo "cnpg role ok"
aws s3 ls | grep core-platform-staging-cnpg-backups

# C1 CMP wiring: the values Secret + the plugin ConfigMap exist in argocd ns
kubectl -n argocd get secret cmp-envsubst-values -o jsonpath='{.data.env}' | base64 -d
kubectl -n argocd get cm argocd-cmp-envsubst -o name
kubectl -n argocd get pod -l app.kubernetes.io/name=argocd-repo-server \
  -o jsonpath='{.items[0].spec.containers[*].name}'   # expect to include "cmp-envsubst"

# C2: Terraform wrote the params file to git
git -C infrastructure fetch && git -C infrastructure show origin/develop:infrastructure/argocd/bootstrap/global-params-staging.json
```

**Do not proceed until** `cmp-envsubst-values` is populated and the repo-server has
the `cmp-envsubst` sidecar — otherwise the workload overlay renders with empty
endpoints.

---

## Phase 2 — ArgoCD platform layer (operators + Karpenter)

The argocd bootstrap (Phase 1) creates the `root-bootstrap` Application, which fans
out the platform/operators/security appsets. Let them sync.

### 2a. Watch the platform appsets render (they read the global-params file)
```bash
kubectl -n argocd get applications | grep -E 'karpenter|lb-controller|external-secrets|cnpg|keda|metrics-server'
argocd app sync root-platform root-operators root-security   # or let auto-sync run
```

### 2b. Validate
```bash
# Karpenter now points at the interruption queue (W5) — value == cluster name
kubectl -n karpenter get deploy karpenter -o yaml | grep -A1 interruptionQueue
# NodePools come ONLY from the Helm chart now (W11) — expect intent-keyed pools, no dup "system"
kubectl get nodepools
kubectl get ec2nodeclass
# CNPG / External Secrets / KEDA operators healthy
kubectl get pods -n cnpg-system -n external-secrets -n keda 2>/dev/null
```

---

## Phase 3 — ArgoCD workload fleet (the staging overlay via the envsubst CMP)

`staging-fleet` syncs `k8s/overlays/staging` through the `envsubst` plugin. This is
where C1/C3–C7/W7/W8/W10 land.

### 3a. Sync + watch the render resolve
```bash
argocd app sync staging-fleet
# Confirm the CMP resolved the endpoints (NOT a literal ${...})
kubectl -n default get cm staging-counter-config -o jsonpath='{.data.KAFKA_BROKERS}'   # real broker DNS
kubectl -n default get svc staging-realtime-gateway-public \
  -o jsonpath='{.metadata.annotations.service\.beta\.kubernetes\.io/aws-load-balancer-ssl-cert}'  # real ACM ARN
```

### 3b. Validate the workload fixes
```bash
# C5: stateful DBs land on the database tier + tolerate its taint
kubectl get pods -n default -l cnpg.io/cluster -o wide          # on intent=database nodes
kubectl get scyllacluster -n scylla -o yaml | grep -A6 placement
# C6: backups configured + a base backup completes
kubectl get scheduledbackups -n default
kubectl get backups -n default                                   # after 02:00 UTC or trigger one:
kubectl cnpg backup staging-audit-postgres
# W7: pods run non-root, no caps
kubectl get pod -n default -l app=audit-server \
  -o jsonpath='{.items[0].spec.securityContext}{"\n"}{.items[0].spec.containers[0].securityContext}'
# W8: NetworkPolicies present AND enforced (cross-ns ingress denied, same-ns allowed)
kubectl get networkpolicy -n default
# W10: counter worker scaler
kubectl get scaledobject staging-counter-worker -n default -o yaml | grep offsetResetPolicy   # earliest
```

### 3c. ⚠️ The two highest-risk changes — verify live

**#519 NetworkPolicy** — default-deny ingress. Watch for severed flows for ~10 min:
```bash
# liveness/readiness still passing (CNI exempts kubelet probes; confirm anyway)
kubectl get pods -n default | grep -v Running
# in-mesh gRPC still works (same-ns allowed). Pick a known edge from the call graph:
#   counter -> social-graph:50053, search -> post:50056/profile:50052, media -> moderation:50061
kubectl -n default logs deploy/staging-counter-server | grep -i 'social-graph\|connection refused'
```
If anything cross-namespace legitimately needs ingress (e.g. a metrics scraper —
this fleet is push-based OTel, so unlikely), add a targeted allow rather than
widening the baseline.

**#509 envsubst CMP** — if any service shows a literal `${VAR}`:
```bash
kubectl -n default get cm -o yaml | grep -n '\${'    # MUST be empty
```
→ the `cmp-envsubst-values` Secret is missing/stale or repo-server didn't pick it
up. Re-check Phase 1c; the CMP sources the Secret file at render time, so a
`argocd app sync staging-fleet --hard-refresh` re-renders after the Secret lands.

---

## Phase 4 — Scylla (applied separately)

The operator-managed `ScyllaCluster` is **not** in the overlay (un-prefixed, namespace
`scylla`). Apply after the scylla-operator is healthy:
```bash
kubectl apply -k k8s/base/infra/scylla-cluster
kubectl get pods -n scylla -o wide    # C5: on intent=database nodes
```

---

## Rollback

All changes are GitOps/IaC — rollback is `git revert` of the offending squash commit
on `develop`, then re-sync. Quick levers per change:

| Change | Fast rollback |
|---|---|
| W8 NetworkPolicy severs traffic | `kubectl delete networkpolicy -n default --all` (instant un-block), then revert #519 |
| C1 CMP renders empty | revert `staging-fleet` to a bare `path` source, or fix the `cmp-envsubst-values` Secret |
| W5 / W3 / W1 (Terraform) | `git revert` the commit, `terragrunt apply`; `moved` blocks mean the VPC NAT change won't recreate |
| C5/C6 CNPG | `serviceAccountTemplate`/`backup` are additive; remove the block + re-sync to stop backups |
| W3 endpoint | if locked out, widen `endpoint_public_access_cidrs` back to `0.0.0.0/0` and apply |

> The single biggest blast-radius item is the **vpc-cni NetworkPolicy enablement** —
> it cannot be rolled back per-pod, only by reverting the addon config (another aws-node roll).

---

## Post-rollout — carry-forward follow-ups (not blocking)

These were deliberately deferred in the PRs:

- **Finish W3:** set `endpoint_public_access_cidrs` per env to your admin/CI ranges
  (commented example in each `eks/terragrunt.hcl`). Today it's still `0.0.0.0/0`.
- **W8 v2:** per-service ingress (isolate TIER-0 audit/auth; workers take no mesh
  ingress) + egress lockdown — needs the **code-level gRPC call graph**.
- **C4 (legacy): resolved** — the legacy Bazel build pipeline is gone; the whole
  fleet is built and SHA-pinned by `fleet-images-deploy.yml`.
- **C6:** CNPG storage still 10Gi (size for prod, esp. the 7-yr audit ledger); wire
  real KMS/HSM + the cross-account WORM witness.
- **W7:** `readOnlyRootFilesystem` per-service (needs scratch-write validation);
  promote the securityContext patch to `base` so dev inherits it.
- **W4:** tighten the node SG 8080 rule from VPC CIDR to the ALB security group.
- **Realtime gateway:** decide on-demand pinning vs spot (nodepool design); W5's
  graceful drain makes spot tolerable meanwhile.
- **W13:** rebuild the prod live tree by mirroring staging when prod is real
  (`infrastructure/live/prod/env.hcl` holds the captured sizing intent).
