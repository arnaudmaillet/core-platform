# Core-Platform — Infrastructure & Operational Documentation

**Document class:** Definitive / Production-grade · **Scope:** `core-platform` monorepo (services, Terragrunt IaC, Kustomize delivery, event topology) · **Primary environment documented:** `staging` (the live Kustomize/GitOps path), with `dev` and `prod` deltas called out · **Status:** Authored from the current `develop` state; staging infrastructure is authored and statically validated but not yet applied to a live cluster.

> **This is the canonical reference.** For task-focused operational deep-dives, see:
> - [GitOps & ArgoCD operations](gitops-argocd.md) — the App-of-AppSets cascade, sync waves, the envsubst CMP, and day-2 ArgoCD ops.
> - [Terragrunt units reference](terragrunt-units.md) — per-unit inputs/outputs/deps, the apply DAG, and exact invocations.
> - [Secret topology (ESO / ClusterSecretStore)](secrets-eso.md) — how a credential travels from Terraform → Secrets Manager → pod env.
> - [Environment lifecycle runbook](../runbooks/environment-lifecycle.md) — preflight → provision → validate → graceful teardown → rebuild.
> - [Documentation entry point & taxonomy](../README.md) — the audience router and the platform/application boundary.

---

## 1. Core Technical Architecture & Deployment Archetypes

### 1.1 Composition model

The platform is a single Rust workspace (`crates/`) compiled into **per-binary container images** via one generic, cache-optimized `deploy/Dockerfile` (`--build-arg BIN=<package>`). Each service is a Domain-Driven, hexagonal crate (`domain → application(ports) → infrastructure(adapters)`) fronted by one or more deployable binaries. Cross-cutting concerns are shared foundation crates (`service-runtime`, `transport` (Kafka + gRPC), `cqrs`, storage adapters for Postgres/Scylla/Redis, `auth-context`, `telemetry`).

Two contract planes govern integration and are both build-gated:

- **Synchronous (gRPC):** versioned `*-api` proto crates, guarded by `buf breaking`.
- **Asynchronous (Kafka):** the `event-topology` registry crate — the single source of truth for producer/consumer wiring, with a contract test that fails the build on a *phantom edge* (a consumer of a topic no producer emits). This is the authoritative map of the streaming fabric (§2.4).

### 1.2 Service tiers & failure semantics

Tier is an explicit runtime contract (`tier:` pod label) that dictates failure posture:

| Tier | Posture | Services | Meaning |
|---|---|---|---|
| **TIER-0** | **Fail-closed** | `auth` (50060), `moderation` (50061), `audit-server` (50068), `audit-worker` (50069) | Identity, trust/safety, and tamper-evident compliance. Correctness over availability — e.g. audit denies an unrecordable privileged write (break-glass). |
| **TIER-1** | **Fail-open** | `counter-server/worker` (50064/50065), `media` (50063), `search` (50062), `realtime-gateway` (8443/50066), `realtime-dispatcher` (50067) | Systems-of-Reference / -Connection / -Delivery. Availability over completeness — degrade gracefully, re-derive from upstream SoRs. |
| **Core (implicit)** | Mixed | `account` (50059), `profile` (50052), `social-graph` (50053), `post` (50056), `comment` (50057), `engagement` (50058), `geo-discovery` (50054), `notification` (50055), `timeline` (50070), `chat` (50051) | The social-graph Systems-of-Record and read-models. |

Internal ports are per-service ClusterIPs; each service now owns a distinct port (`timeline` moved 50060 → 50070 to clear its reuse of `auth`'s port).

### 1.3 Deployment archetypes

The fleet resolves to **four** reusable archetypes, distinguished by runtime characteristic rather than domain:

1. **RPC Server** — request-bound, gRPC, CPU-scaled. Readiness gates on typed gRPC health (`SERVING` only once backend probes pass); liveness checks process liveness only, so a transient backend blip drops a pod from rotation without a restart loop. *(all `*-server` binaries)*
2. **Stream Worker** — Kafka-consumer-bound, no domain RPC (health/reflection plane only), scaled on **consumer-group lag**. *(`counter-worker`, `audit-worker`, `realtime-dispatcher`)*
3. **Stateful Edge** — `realtime-gateway`: owns a long-lived connection table (one parked-future socket per device, C10M design), scaled on connections/memory, protected by a **PodDisruptionBudget** to make drains gradual, exposed publicly via an **L4 NLB** (never an ALB — see §2.5).
4. **Migration init-container** — every Postgres/Scylla-backed service runs the shared `migrator` as an idempotent init container (`args: [<service>]`) before the runtime container boots; dual-store services (`counter`, `moderation`) run one per backend.

### 1.4 Scaling model

| Mechanism | Trigger | Workloads |
|---|---|---|
| **HPA** | CPU | `auth`, `moderation`, `counter-server`, `media`, `search`, `realtime-gateway`, `audit-server` |
| **KEDA ScaledObject** | Kafka consumer-group lag | `counter-worker` (max 12), `realtime-dispatcher` (max 8), `audit-worker` (max 4) |
| **PDB** | voluntary-disruption floor | `realtime-gateway` (minAvailable 1) |

KEDA is a hard prerequisite (operator at GitOps sync-wave −10). A Kustomize `nameReference` extension propagates the env `namePrefix` into the `ScaledObject.scaleTargetRef` and `TriggerAuthentication.authenticationRef` (the built-in transformer covers HPA but not the `keda.sh` CRDs) — without it a prefixed scaler silently targets a non-existent Deployment. **Lag-scaler maxReplicaCount must never exceed the topic partition count** (a consumer group cannot parallelize beyond its partitions).

---

## 2. AWS & Repo Infrastructure State

### 2.1 Environment topology

| Env | Backends | Delivery path | State |
|---|---|---|---|
| **dev** | In-cluster (Redpanda, ScyllaDB StatefulSet, per-service Redis, account Postgres) | Legacy ArgoCD Helm catalog (`apps/catalog`), `profile` only live; Kustomize `overlays/dev` for local | Partial |
| **staging** | **Managed AWS** (MSK, ElastiCache, OpenSearch, S3, KMS) + in-cluster operators (scylla-operator, CNPG) | **`staging-fleet` ArgoCD Application → `k8s/overlays/staging`** (Kustomize) | Authored, validated, **not applied** |
| **prod** | **Managed AWS mirror of staging** with the production posture (3 AZ + NAT-per-AZ, 3-broker MSK, COMPLIANCE WORM, nothing disposable) | **`prod-fleet` ArgoCD Application → `k8s/overlays/prod`**, tracking **`main`** (merging develop → main is the prod deploy) | Scaffolded, **not applied** (prereqs: `live/prod/env.hcl`) |

All environments target AWS account `724772065879` / `us-east-1`, sharing one ECR registry (one repo per binary, env-tagged).

### 2.2 Terraform modules & Terragrunt structure

**Modules** (`infrastructure/modules/`): `networking/{vpc,route53}`, `eks`, `artifacts/ecr`, `security/irsa-roles`, `kubernetes/argocd`, `elasticache`, `msk`, `opensearch`, `s3-bucket` (generic; Object-Lock parameter), `kms-key`.

**Terragrunt live tree** (`infrastructure/live/<env>/us-east-1/`): `networking/vpc → eks → data/{msk,elasticache,opensearch,media-bucket,audit-kms,audit-worm} → security/irsa-roles → kubernetes/argocd`. Remote state (S3 + lockfile) and providers are generated centrally by `root.hcl`. `global/artifacts/ecr` is the account-shared, authoritative registry list (all fleet binaries + `migrator` + `buildcache`).

### 2.3 Managed data stores (staging)

| Store | Module | Purpose | Credential path |
|---|---|---|---|
| **MSK (Kafka)** | `msk` | Async event fabric; SASL/SCRAM over TLS | SCRAM secret → Secrets Manager → ESO → `backend-creds` |
| **ElastiCache (Redis)** | `elasticache` | Hot tiers, presence/registry, fan-out; cluster mode + TLS + AUTH | AUTH token → SM → ESO → `backend-creds` |
| **OpenSearch** | `opensearch` | `search` inverted index (SoReference); VPC, TLS, fine-grained access | Master user → SM → ESO → `search-creds` |
| **S3 — media** | `s3-bucket` | Asset bytes; versioned, SSE-S3, CORS for presigned upload/download | Static keys → SM → ESO → `media-s3-creds` |
| **S3 — audit WORM** | `s3-bucket` | Compliance evidence anchor; **Object-Lock COMPLIANCE** + SSE-KMS | Static keys + KEK → SM → ESO → `audit-crypto` |
| **KMS** | `kms-key` | Audit KEK (wraps per-subject DEKs; GDPR crypto-shred) | IRSA-scoped (sole principal) |
| **CNPG Postgres ×6** | in-cluster (overlay) | `account`, `counter` (warm ledger), `audit` (chain), `moderation`, `auth`, `media` | Cluster-generated `<name>-app` secret (`uri`) |
| **ScyllaDB** | scylla-operator | `counter` (cold TWCS), `moderation` (history) + core social-graph keyspaces | In-cluster, no auth |

### 2.4 Asynchronous pipeline topology

Extracted from the `event-topology` registry (the build-enforced source of truth). Principal flows:

- **Compliance ingest (TIER-0):** `account.v1.events`, `auth.v1.events`, `moderation.v1.events` → **`audit`**. Audit is a terminal sink; it also self-consumes a generic `audit.v1.events` ingest lane fed by the synchronous `RecordPrivileged` gRPC path.
- **Discovery read-model:** `profile.v1.events` + `post.v1.events` + `moderation.v1.events` → **`search`** (index + visibility).
- **Engagement → magnitudes:** `engagement.reactions` → **`counter`** (aggregation), `notification`, self (write-behind). Counter also consumes deferred upstream telemetry (`view/impression/click.v1.events`).
- **Counter → live + virality:** `counter.v1.popularity` → **`realtime`** (broadcast) + **`geo-discovery`** (re-score).
- **Social fan-out:** `social-graph.followed/unfollowed` → **`timeline`**; `social-graph.author_tier_changed` → **`profile`** (tier ownership).
- **Live push:** `post.v1.events` → **`realtime`**; `media.v1.events` self-consumed (Plane-B transform); `moderation.v1.events` → **`media`** (takedown).

The registry also formally tracks **DEFERRED** consumers (external/un-built producers: `moderation.reports/signals`, `view/impression/click.v1.events`, the `social-graph.follows` naming mismatch) and **ORPHAN_PRODUCERS** (intentional headroom: legacy `post.updated`, `social-graph.blocked` enforced on the read path, the chat delivery-plane topics). Note: the registry guards topic **wiring**, not payload **shape** — a known `post → geo/notification` payload gap remains a separate, tracked concern.

The registry is also the **broker provisioning source**: the `topic-provisioner` binary (ArgoCD PreSync hook Job in each overlay) creates every stream topic plus its `.dlq` counterpart in one idempotent admin call. MSK runs with `auto.create.topics.enable=false` (explicit server property), so a topic exists **because** it is in the registry — a typo'd topic name fails the sync instead of spawning a phantom topic with defaults nobody chose.

### 2.5 TIER-0 security & compliance control boundaries

- **Audit immutability** is enforced across four independent trust domains: (1) application-level INSERT-only hash-chained ledger (Postgres); (2) **S3 Object-Lock in COMPLIANCE mode** (write-once, not deletable even by the account root before retention); (3) SSE-KMS under a dedicated **KEK**; (4) signed Merkle checkpoints anchored to the WORM bucket as an external witness.
- **Least-privilege custody (IRSA):** the `audit` role is the *sole principal* granted `kms:Decrypt/GenerateDataKey` on the KEK and **write-only (no `DeleteObject`)** on the WORM bucket; the `media` role is scoped to object RW on its bucket only. Roles are created only when their resource ARNs exist (dev unaffected).
- **GDPR Art. 17:** crypto-shred — destroy a subject's per-subject DEK; the chain (over ciphertext) still verifies, the evidence and its proof survive, legal-hold overrides.
- **Fail-closed posture:** TIER-0 services deny rather than degrade (e.g. break-glass refused if the write is unrecorded).
- **Edge isolation:** the only public ingress is the realtime WSS plane via an **L4 NLB** (TLS terminated at the edge, raw WS handshake reaches the pod) — deliberately *not* an L7 ALB, so the gateway, not a proxy, owns the connection table.

### 2.6 GitOps delivery

ArgoCD App-of-AppSets, bootstrapped per environment (`bootstrap/` and `bootstrap/staging/`): **operators** (CNPG, scylla-operator, External Secrets, **KEDA**, k6) at sync-wave **−10**, then **security** (cert-manager, AWS LB controller, external-dns), **platform** (Karpenter, metrics-server), **observability** (monitoring), and **workloads** at wave **0**. Sequencing is load-bearing: operator CRDs (`ScaledObject`, `Cluster`, `ExternalSecret`) must exist before the workload overlay applies the CRs that reference them.

---

## 3. Deployment & Bootstrapping Runbook (staging → production)

### 3.1 Provisioning dependency order

Terragrunt resolves the DAG with `run-all apply`; the explicit order (each consuming the prior's outputs) is:

```
1. networking/vpc                      # VPC, subnets, CIDR
2. eks                                  # cluster, OIDC provider, node groups (system + database)
3. data/msk                            # Kafka brokers + SCRAM secret
   data/elasticache                    # Redis endpoint + AUTH secret
   data/opensearch                     # domain + master secret
   data/media-bucket                   # media S3 bucket
   data/audit-kms                      # audit KEK
   data/audit-worm                     # Object-Lock bucket (depends on audit-kms)
4. security/irsa-roles                 # ESO + audit/media app roles — CONSUMES the data ARNs, so AFTER step 3
5. kubernetes/argocd                   # ArgoCD; writes global-params-staging.json
6. (GitOps) operators converge (wave -10): CNPG, scylla-operator, ESO, KEDA
7. kubectl apply -k k8s/base/infra/scylla-cluster   # ScyllaCluster in ns `scylla` (un-prefixed FQDN)
8. (GitOps) workloads sync (wave 0): k8s/overlays/staging
```

**Critical ordering note:** `security/irsa-roles` now depends on the `audit-kms`/`audit-worm`/`media-bucket` outputs — it must run **after** the data stores (a reordering from the historical layout; `mock_outputs` permit a dry `plan` beforehand).

### 3.2 Secrets & credential management

- **Machine-generated (Terraform → Secrets Manager → ESO):** MSK SCRAM, Redis AUTH, OpenSearch master. Synced into `backend-creds` (fleet-wide `envFrom`) and `search-creds`.
- **`DATABASE_URL`:** sourced from each CNPG cluster's generated `<name>-app` secret (`uri` key), injected via per-service patches into **both** the migrator init container **and** the runtime container (the migrator hard-requires it).
- **Seeded out-of-band (must be created manually in Secrets Manager under `core-platform-staging-*`):**
  - `…-media-s3` `{access_key, secret_key}` — IAM-user static keys.
  - `…-audit-crypto` `{object/witness access+secret, kek_base64, signing_key_base64}`.
  - `…-auth-secrets` `{signing_private_pem, signing_public_pem, keycloak_client_secret}`.

### 3.3 Manual steps & placeholders

Endpoint placeholders (`<<…>>`) are substituted from Terragrunt outputs at deploy time (in `.env` files and the KEDA scaler patches): `<<MSK_BOOTSTRAP_BROKERS_SASL_SCRAM>>`, `<<ELASTICACHE_CONFIG_ENDPOINT>>`, `<<OPENSEARCH_ENDPOINT>>`, `<<ACM_CERTIFICATE_ARN>>` (NLB TLS), `<<KEYCLOAK_TOKEN_ENDPOINT>>`, `<<AUTH_JWKS_URL>>`. Additionally: seed the §3.2 secrets; verify each lag-scaled topic has **≥ maxReplicaCount partitions** (`counter`=12, `realtime`=8, `audit`=4); build/push images via the `fleet-images-deploy` matrix CI to `:staging`.

### 3.4 Day-1 caveats & known deferrals

1. **Object-store credential model (action required):** `media` and `audit` construct their S3/KMS clients with **static `rusty-s3` credentials**, *not* the AWS SDK credential chain — so the IRSA roles, while provisioned, are **not consumed by the code as-built**. Staging requires IAM-user static keys seeded into `media-s3-creds`/`audit-crypto`. The IRSA roles remain the correct target should the code migrate to the SDK.
2. **Audit external deferrals:** real AWS KMS and a true cross-account WORM witness are deferred (IAM/org work); the staging wiring uses the **v1 ENV-KEK path** with the witness pointed at the same WORM bucket.
3. **Keycloak not provisioned:** `auth`'s federated IdP is external and not yet stood up; its broker creds are placeholders. `realtime`'s WSS plane fails closed (`RTM-1001`) until auth's JWKS is reachable — its gRPC health plane is unaffected, so the pod still becomes Ready.
4. **Mutable `:staging` tag:** ArgoCD will not auto-redeploy on a tag re-push without Argo Image Updater or a digest bump; the `:<git-sha>` tag is available for pinning.
5. **Streaming payload gap:** the `post → geo-discovery/notification` payload-shape mismatch (post emits no lat/lng/caption) remains an open, tracked product decision — wiring is correct, shape is not.

---

## Appendix A — Port allocation

`chat` 50051 · `profile` 50052 · `social-graph` 50053 · `geo-discovery` 50054 · `notification` 50055 · `post` 50056 · `comment` 50057 · `engagement` 50058 · `account` 50059 · `auth` 50060 · `timeline` 50070 · `moderation` 50061 · `search` 50062 · `media` 50063 · `counter-server` 50064 · `counter-worker` 50065 · `realtime-gateway` 50066 (gRPC) + 8443 (WSS) · `realtime-dispatcher` 50067 · `audit-server` 50068 · `audit-worker` 50069.

## Appendix B — Topic catalog

**Producers:** `account.v1.events`, `profile.v1.events`, `post.{published,updated,deleted,v1.events}`, `comment.{created,deleted}`, `engagement.reactions`, `social-graph.{followed,unfollowed,blocked,author_tier_changed}`, `chat.*`, `counter.v1.popularity`, `moderation.v1.events`, `auth.v1.events`, `media.v1.events`. **Deferred consumers:** `audit.v1.events`, `moderation.{reports,signals}`, `view/impression/click.v1.events`, `social-graph.follows`. **Orphan producers (headroom):** `post.updated`, `social-graph.blocked`, `chat.{conversation.created,conversation.published,member.joined,member.left,message.sent}`.
