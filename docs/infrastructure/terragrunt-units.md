# Terragrunt Units Reference

**Document class:** Operational / Production-grade · **Audience:** DevOps &
platform engineers · **Scope:** `infrastructure/live/staging/us-east-1` (the live
region tree) · **Companion to:** the [infrastructure master guide](README.md) and
the [GitOps guide](gitops-argocd.md).

This is the per-unit reference for the Terragrunt live tree: what each unit
provisions, what it depends on, what it outputs downstream, and the exact
`plan`/`apply`/`destroy` invocations. The order here is the **apply DAG** — units
consume the outputs of the units above them.

---

## 1. Structure & conventions

```
infrastructure/
├── modules/                    # reusable Terraform modules (the "how")
│   ├── networking/{vpc,route53}   eks   acm-cert   artifacts/ecr
│   ├── elasticache   msk   opensearch   s3-bucket (generic; Object-Lock param)
│   ├── kms-key   app-secrets   security/irsa-roles   kubernetes/argocd
└── live/                       # Terragrunt instantiations (the "where/which")
    ├── global/{artifacts/ecr, networking/route53}     # account-shared
    ├── dev/us-east-1/…
    ├── staging/us-east-1/…     # ◄── documented here (the live path)
    └── prod/us-east-1/…        # STUB (eks + networking only)
```

- **`root.hcl`** (parent) generates the S3 remote-state backend + lockfile and the
  AWS provider block for every unit centrally. Individual units never redeclare
  them.
- **`region.hcl`** carries `aws_region`. Units read it via
  `read_terragrunt_config(find_in_parent_folders("region.hcl"))`.
- **`dependency` blocks** wire unit-to-unit output consumption and *define the DAG*.
  Most data-store dependencies carry `mock_outputs` gated to
  `["validate","plan"]`, so a `run-all plan` works before the real resources exist
  (first-run bootstrap, or after a full teardown).
- **One module can back many units.** `s3-bucket` backs three different units
  (`media-bucket`, `audit-worm`, `cnpg-backups`) with different parameters —
  Object-Lock on for audit, off for media.

---

## 2. The apply DAG (staging region tree)

Thirteen units resolve in this dependency order. `terragrunt run-all apply` walks
the DAG automatically; the numbering shows the levels that can run concurrently.

```
Level 0 (no deps):   networking/vpc     networking/acm-cert     data/media-bucket
                     data/audit-kms      data/cnpg-backups
        │
Level 1:   eks ─────────────────────────► (vpc)
           data/msk  data/elasticache  data/opensearch ──► (vpc)
           data/audit-worm ─────────────► (audit-kms)
        │
Level 2:   data/app-secrets ────────────► (media-bucket, audit-worm, audit-kms)
        │
Level 3:   security/irsa-roles ─────────► (eks, audit-kms, audit-worm,
        │                                   media-bucket, cnpg-backups)
        │
Level 4:   kubernetes/argocd ───────────► (vpc, eks, security/irsa-roles,
                                            msk, elasticache, opensearch,
                                            acm-cert, app-secrets)
```

> **The load-bearing edge:** `security/irsa-roles` depends on the **data-store
> ARNs** (audit KMS/WORM, media/cnpg buckets), so it must apply **after** the data
> stores — a reordering from the historical layout where IRSA came earlier. The
> `mock_outputs` on those dependencies let a dry `plan` run before they exist.
>
> **Why `acm-cert` is its own Level-0 unit:** it was decoupled from `eks` (PR #543)
> so that tearing down the certificate can't leave a dangling reference that leaks
> the VPC. The cert is consumed by the `kubernetes/argocd` unit (NLB TLS listener),
> not by `eks`.

---

## 3. Per-unit reference

Legend: **Module** = backing module · **Depends on** = units consumed · **Key
outputs** = what downstream units read.

### Networking & compute

| Unit | Module | Depends on | Provisions / Key outputs |
|---|---|---|---|
| **`networking/vpc`** | `networking/vpc` | — | VPC, public/private subnets, CIDR, NAT. → `vpc_id`, subnet IDs. Consumed by nearly everything. |
| **`networking/acm-cert`** | `acm-cert` | — | Public ACM cert for the NLB WSS listener. → `certificate_arn`. Decoupled from `eks` (#543). |
| **`eks`** | `eks` | `vpc` | EKS cluster, **OIDC provider** (IRSA trust anchor), managed node groups (system + database). → `cluster_name`, `cluster_endpoint`, `cluster_certificate_authority_data`, OIDC issuer. |

### Data plane (managed AWS stores)

| Unit | Module | Depends on | Provisions / Key outputs |
|---|---|---|---|
| **`data/msk`** | `msk` | `vpc` | MSK (Kafka), SASL/SCRAM over TLS + SCRAM secret. → `bootstrap_brokers_sasl_scram`. |
| **`data/elasticache`** | `elasticache` | `vpc` | ElastiCache Redis, cluster-mode + TLS + AUTH. → `configuration_endpoint`. |
| **`data/opensearch`** | `opensearch` | `vpc` | OpenSearch domain (VPC, TLS, fine-grained access) for `search`. → `endpoint`. |
| **`data/media-bucket`** | `s3-bucket` | — | Media asset bucket: versioned, SSE-S3, CORS for presigned upload/download. → bucket ARN/name. |
| **`data/audit-kms`** | `kms-key` | — | Audit **KEK** (wraps per-subject DEKs; GDPR crypto-shred). → key ARN. Sole principal is the audit IRSA role. |
| **`data/audit-worm`** | `s3-bucket` | `audit-kms` | Compliance evidence bucket: **Object-Lock COMPLIANCE** + SSE-KMS under the audit KEK. → bucket ARN. |
| **`data/cnpg-backups`** | `s3-bucket` | — | Backup target for the in-cluster CNPG Postgres clusters. → bucket ARN. |
| **`data/app-secrets`** | `app-secrets` | `media-bucket`, `audit-worm`, `audit-kms` | Seeds/organizes the Secrets Manager entries the fleet's ExternalSecrets pull. Ordering-only for the argocd unit (`skip_outputs`). |

### Security & delivery

| Unit | Module | Depends on | Provisions / Key outputs |
|---|---|---|---|
| **`security/irsa-roles`** | `security/irsa-roles` | `eks`, `audit-kms`, `audit-worm`, `media-bucket`, `cnpg-backups` | The IRSA roles: ESO, Karpenter, LB controller, external-dns, cert-manager, EBS CSI, and the app roles (audit=sole KMS/WORM principal, media=bucket RW). → per-role ARNs. |
| **`kubernetes/argocd`** | `kubernetes/argocd` | `vpc`, `eks`, `security/irsa-roles`, `msk`, `elasticache`, `opensearch`, `acm-cert`, `app-secrets` | Installs ArgoCD + `root-bootstrap` (targets `bootstrap/staging`). Writes **`cmp-envsubst-values`** (data-store endpoints for the CMP) and **`global-params-staging.json`**. Carries the **graceful-cleanup `before_hook` on `destroy`**. |

> The `kubernetes/argocd` unit is the seam between Terraform and GitOps: it is the
> *last* Terraform unit and the *first* thing that hands control to ArgoCD (see the
> [GitOps guide §4](gitops-argocd.md#4-bootstrap-order--terraform-then-gitops-converges)).

---

## 4. Command reference

Run from the region root unless targeting a single unit.

```bash
BASE=infrastructure/live/staging/us-east-1

# --- Whole tree ---
( cd $BASE && terragrunt run-all plan )                      # dry-run the DAG (mock_outputs cover unbuilt stores)
( cd $BASE && AWS_REGION=us-east-1 GITHUB_TOKEN=$(gh auth token) \
    terragrunt run --all apply --non-interactive --backend-bootstrap -- -auto-approve )
( cd $BASE && AWS_REGION=us-east-1 \
    terragrunt run --all destroy --non-interactive -- -auto-approve )

# --- Single unit (e.g. re-write the CMP values Secret after a data-store change) ---
( cd $BASE/kubernetes/argocd && terragrunt apply )
( cd $BASE/data/msk && terragrunt plan )
( cd $BASE/data/msk && terragrunt output )                   # inspect a unit's outputs

# --- Global (account-shared) ---
( cd infrastructure/live/global/artifacts/ecr && terragrunt apply )   # the authoritative ECR repo list
```

> **Trust the Run Summary, not the exit code.** `terragrunt run-all` / `run --all`
> can exit `0` even when individual units report `Failed`. Read the
> `Succeeded / Failed` summary at the end of the run.

### The `GITHUB_TOKEN` on apply

The `kubernetes/argocd` unit registers the repo with ArgoCD; the apply needs a
GitHub token in the environment (`GITHUB_TOKEN=$(gh auth token)`). Omit it and the
ArgoCD repo registration step fails while the AWS resources still apply — leaving a
half-bootstrapped cluster.

---

## 5. Teardown ordering & the graceful-cleanup hook

`destroy` walks the DAG in **reverse**, so `kubernetes/argocd` is torn down first —
which is exactly where the `before_hook "graceful_cleanup"` fires. That hook
(`infrastructure/assets/teardown/k8s-graceful-cleanup.sh`) drains the AWS resources
that **in-cluster controllers** created and Terraform does **not** own — ALBs/NLBs
(and their ENIs), CNPG/Scylla EBS volumes, and Karpenter EC2 nodes — **before**
Terraform deletes the cluster and VPC. Without it those resources leak and leftover
LB ENIs block `aws_vpc` destroy with `DependencyViolation`.

The full teardown → rebuild loop, including the Secrets-Manager/KMS deletion-state
gotchas that outlive a `destroy`, is documented in the
[environment lifecycle runbook](../runbooks/environment-lifecycle.md) and the
[disposable staging rebuild runbook](../runbooks/staging-disposable-rebuild.md).

---

## 6. `dev` and `prod` deltas

- **`dev`** — same module set, but data stores are **in-cluster** (Redpanda,
  ScyllaDB StatefulSet, per-service Redis, account Postgres) rather than managed
  AWS; no MSK/ElastiCache/OpenSearch/KMS/WORM units. Delivery is the legacy Helm
  catalog (`profile-service` only) plus `overlays/dev` for local iteration.
- **`prod`** — **STUB**: only `eks` + `networking` are authored. No data plane, no
  GitOps bootstrap yet. There is no separate prod cluster; `staging` is the active
  path.

---

## Appendix — module ↔ unit matrix

| Module | Units instantiating it |
|---|---|
| `networking/vpc` | `networking/vpc` |
| `acm-cert` | `networking/acm-cert` |
| `eks` | `eks` (staging, dev, prod-stub) |
| `msk` / `elasticache` / `opensearch` | `data/msk` · `data/elasticache` · `data/opensearch` |
| `s3-bucket` (generic) | `data/media-bucket` (Lock off) · `data/audit-worm` (Lock COMPLIANCE) · `data/cnpg-backups` |
| `kms-key` | `data/audit-kms` |
| `app-secrets` | `data/app-secrets` |
| `security/irsa-roles` | `security/irsa-roles` |
| `kubernetes/argocd` | `kubernetes/argocd` |
| `artifacts/ecr` | `global/artifacts/ecr` (account-shared) |
| `networking/route53` | `global/networking/route53` (account-shared) |
