# Secret Topology — External Secrets Operator & ClusterSecretStore

**Document class:** Operational / Production-grade · **Audience:** DevOps
engineers **and** service authors (both cross this boundary) · **Scope:**
`staging` credential plane · **Companion to:** the
[infrastructure master guide](README.md), [GitOps guide](gitops-argocd.md), and
[Terragrunt units reference](terragrunt-units.md).

This document is the single source of truth for how a credential travels from AWS
into a pod's environment, the machine-generated-vs-seeded split, and the exact
steps to add a new secret. If you are debugging a missing env var or wiring a new
managed backend, start here.

---

## 1. The pipeline in one line

```
Terraform ──► AWS Secrets Manager ──► [ESO reads via IRSA] ──► k8s Secret ──► pod env (envFrom)
   (writes/seeds SM entries)   (ClusterSecretStore + ExternalSecret)     (deployment patch)
```

Nothing in the cluster ever talks to AWS Secrets Manager except the **External
Secrets Operator (ESO)**. Services read plain Kubernetes Secrets and never hold AWS
credentials for secret retrieval — that is the whole point of the topology.

```
┌─ AWS ────────────────────────────────────────────────────────────────────┐
│  Secrets Manager                                                          │
│    AmazonMSK_core-platform-staging_app        {username, password}        │
│    core-platform-staging-redis-auth           {password}                  │
│    core-platform-staging-opensearch-master    {username, password}        │
│    core-platform-staging-media-s3             {access_key, secret_key}    │
│    core-platform-staging-audit-crypto         {object/witness keys, kek…} │
│    core-platform-staging-auth-secrets         {signing pems, kc secret}   │
└───────────────┬───────────────────────────────────────────────────────────┘
                │  ESO IRSA role (external_secrets) assumed by the
                │  external-secrets ServiceAccount (OIDC/JWT)
                ▼
┌─ cluster ─────────────────────────────────────────────────────────────────┐
│  ClusterSecretStore  "aws-secrets-manager"  (cluster-scoped, one per env)  │
│      │                                                                     │
│      ├─ ExternalSecret backend-creds   ──► Secret backend-creds  (fleet)   │
│      ├─ ExternalSecret search-creds     ──► Secret search-creds   (search) │
│      ├─ ExternalSecret media-s3-creds   ──► Secret media-s3-creds (media)  │
│      ├─ ExternalSecret audit-crypto     ──► Secret audit-crypto   (audit)  │
│      └─ ExternalSecret auth-secrets     ──► Secret auth-secrets   (auth)   │
└───────────────┬───────────────────────────────────────────────────────────┘
                │  envFrom (deployment patch)
                ▼
            service pod environment
```

Manifests: `k8s/overlays/staging/external-secrets.yaml`. Refresh interval is **1h**
per ExternalSecret — a rotated SM value propagates to the pod's Secret within the
hour (a pod restart picks it up immediately via envFrom).

---

## 2. Why `ClusterSecretStore`, not a namespaced `SecretStore`

This is a deliberate, load-bearing choice (documented inline in the manifest):

- The ESO IRSA **ServiceAccount lives in the `external-secrets` namespace**.
- A namespaced `SecretStore` may only reference a ServiceAccount **in its own
  namespace** — ESO's admission webhook rejects a cross-namespace
  `serviceAccountRef` (`"namespace should be empty or match the SecretStore's
  namespace"`).
- A **`ClusterSecretStore` is cluster-scoped** and may reference the operator's SA
  in `external-secrets` — the canonical IRSA pattern. So the fleet's ExternalSecrets
  (which live in the workload namespace) all point at the one cluster-scoped store.

The store authenticates with `auth.jwt.serviceAccountRef` → the `external-secrets`
SA, whose IRSA role (`enable_external_secrets` in `modules/security/irsa-roles`)
grants read on `core-platform-staging-*` (and the `AmazonMSK_*` secret).

> **No sync-wave on the store.** ESO **re-reconciles** ExternalSecrets once the store
> exists, so gating the fleet on the store was unnecessary — and worse, a prior
> `sync-wave: -1` deployed **zero pods** whenever the store failed to apply. Let ESO
> converge instead of ordering it.

---

## 3. The two credential classes

| Class | How it gets into Secrets Manager | Examples |
|---|---|---|
| **Machine-generated** | Written by the **data-store Terraform modules** at apply time. | MSK SCRAM (`AmazonMSK_…_app`), Redis AUTH (`…-redis-auth`), OpenSearch master (`…-opensearch-master`). |
| **Seeded** | Provisioned by the **`data/app-secrets`** Terragrunt unit (formerly created out-of-band by hand). | `…-media-s3`, `…-audit-crypto`, `…-auth-secrets`. |

Both classes end up as SM entries under the `core-platform-staging-*` (or
`AmazonMSK_core-platform-staging_*`) prefix, and ESO reads them uniformly. The split
matters only at *provisioning* time: machine-generated values appear as a side
effect of applying the data unit; seeded values are the `data/app-secrets` unit's
job. See the [Terragrunt units reference](terragrunt-units.md#data-plane-managed-aws-stores).

---

## 4. The five ExternalSecrets (what each feeds)

| ExternalSecret → k8s Secret | Consumed by | Keys (env var ← SM property) |
|---|---|---|
| **`backend-creds`** (fleet-wide `envFrom`) | every service touching Kafka/Redis | `KAFKA_SASL_USERNAME/PASSWORD` ← MSK app secret; `REDIS_PASSWORD` ← redis-auth |
| **`search-creds`** | `search` only | `SEARCH_OPENSEARCH_USER/PASSWORD` ← opensearch-master |
| **`media-s3-creds`** | `media` only | `MEDIA_S3_ACCESS_KEY/SECRET_KEY` ← media-s3 |
| **`audit-crypto`** | `audit` only | object+witness access/secret keys, `AUDIT_KEK_BASE64`, `AUDIT_CHECKPOINT_SIGNING_KEY_BASE64` ← audit-crypto |
| **`auth-secrets`** | `auth` only | `AUTH_SIGNING_PRIVATE/PUBLIC_PEM`, `AUTH_KEYCLOAK_CLIENT_SECRET` ← auth-secrets |

Two conventions that trip people up:

- **Target names are literal** — Kustomize does not prefix fields *inside* a CR, so
  the `target.name` is spelled out (e.g. `backend-creds`) to match the `envFrom` in
  the deployment patch exactly. The Kustomize `nameReference` transformer doesn't
  reach into ExternalSecret bodies; the `external-secrets-refs-config.yaml`
  configuration entry handles what it can.
- **`secretKey` *is* the env var name** for envFrom-mounted secrets — it must match
  what the service reads in code, character for character.

---

## 5. The IRSA vs static-keys caveat (read before touching media/audit)

**Not every "IRSA role" is actually consumed by the code.** `media` and `audit`
construct their S3/KMS clients with **static `rusty-s3` SigV4 credentials**, *not*
the AWS SDK credential chain — so although the `media`/`audit` IRSA roles are
provisioned, the code as-built does **not** assume them for object-store access.
That is why `media-s3-creds` and `audit-crypto` carry **static IAM-user access
keys** seeded via `data/app-secrets`.

- The IRSA roles remain the correct target *should the code migrate to the SDK* —
  this is a tracked deferral, not a bug.
- ESO itself **does** use IRSA (its SA assumes the `external_secrets` role). The
  distinction is: ESO uses IRSA to *read secrets*; media/audit use *static keys from
  those secrets* to reach S3.

For `audit`, real AWS KMS/HSM custody and a true cross-account WORM witness are the
documented external deferral; the wiring here is the **v1 ENV-KEK path**
(`AUDIT_KEK_BASE64`) with the witness pointed at the same WORM bucket.

---

## 6. Adding a new secret (the exact procedure)

This crosses the platform/application boundary — three ordered edits, one per layer:

**A. Provision the SM entry (platform).**
- *Machine-generated?* It appears when the data module applies — nothing to add.
- *Seeded?* Add it to the **`data/app-secrets`** unit so Terraform writes it under
  `core-platform-staging-<name>`. Never `aws secretsmanager create-secret` by hand
  for a permanent secret — it won't survive a rebuild.

**B. Project it into the cluster (platform).** Add an `ExternalSecret` to
`k8s/overlays/staging/external-secrets.yaml`:

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ExternalSecret
metadata:
  name: my-new-creds
spec:
  refreshInterval: 1h
  secretStoreRef: { name: aws-secrets-manager, kind: ClusterSecretStore }
  target: { name: my-new-creds }              # literal — match the envFrom below
  data:
    - secretKey: MY_SERVICE_TOKEN             # == the env var name the service reads
      remoteRef: { key: "core-platform-staging-my-thing", property: token }
```

The ESO read policy already covers `core-platform-staging-*`, so **no IAM change**
is needed for a secret under that prefix. A new prefix requires widening the
`external_secrets` role policy in `modules/security/irsa-roles`.

**C. Consume it (application).** Add an `envFrom` patch to the service's deployment
pointing at the `my-new-creds` Secret, and read `MY_SERVICE_TOKEN` from the
environment in code. This is the only step a service author owns.

Then validate the render and let GitOps converge:

```bash
kubectl kustomize k8s/overlays/staging | grep -A3 my-new-creds   # renders?
# merge to develop → ArgoCD syncs → ESO materializes the Secret
```

---

## 7. Operating & debugging

```bash
# Is the store healthy?
kubectl get clustersecretstore aws-secrets-manager -o wide
kubectl describe clustersecretstore aws-secrets-manager | tail -20

# Did an ExternalSecret sync? (SecretSynced=True is the goal)
kubectl get externalsecret -A
kubectl describe externalsecret backend-creds        # events show SM read failures

# Did the target k8s Secret materialize with the right keys?
kubectl get secret backend-creds -o jsonpath='{.data}' | jq 'keys'

# ESO operator logs (AccessDenied, secret-not-found, KMS state)
kubectl -n external-secrets logs deploy/external-secrets | tail -50
```

### Failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `ExternalSecret` `SecretSyncedError`, `AccessDenied` | ESO IRSA role can't read the SM key (wrong prefix / policy) | Confirm the key is under `core-platform-staging-*`; else widen the `external_secrets` policy. |
| `SecretSyncedError`, `ResourceNotFoundException` | SM entry doesn't exist (seeded secret never provisioned) | Apply `data/app-secrets`; check the property names match. |
| `SecretSyncedError`, `PendingDeletion` after a rebuild | SM name reserved by a prior teardown | Run `preflight-clean-env.sh staging --fix`, wait, re-apply — see the [lifecycle runbook](../runbooks/environment-lifecycle.md). |
| Pod env has the var but value is wrong/empty | `property` name mismatch, or SM value seeded blank (e.g. Keycloak placeholder) | Fix the `remoteRef.property`; note `AUTH_KEYCLOAK_CLIENT_SECRET` is a **placeholder** until Keycloak lands (DEFERRED). |
| Store `ValidationFailed`, `serviceAccountRef` rejected | Someone changed it to a namespaced `SecretStore` | Must be a `ClusterSecretStore` (§2). |

---

## 8. Boundary summary

- **Platform owns:** Secrets Manager entries (Terraform), the ESO IRSA role, the
  `ClusterSecretStore`, and the `ExternalSecret` definitions.
- **Service authors own:** the `envFrom` patch and reading the env var in code — and
  choosing the env var name, which must equal the `secretKey`.
- **The contract between them** is the k8s Secret name + its keys. Agree on those,
  and neither side needs the other's internals.
