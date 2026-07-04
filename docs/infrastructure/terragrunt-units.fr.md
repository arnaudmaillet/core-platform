---
i18n:
  source: ./terragrunt-units.md
  source_sha256: 4b2addb34a854c781de7915bd3abb4dc7eb87b593fa25819c357b6d4fda764fe
  translated_at: 2026-07-04
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`terragrunt-units.md`](./terragrunt-units.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, noms de topics, signatures, identifiants) sont volontairement
> laissés en anglais.

# Référence des unités Terragrunt

**Classe de document :** Opérationnel / Production · **Audience :** ingénieurs DevOps
& plateforme · **Périmètre :** `infrastructure/live/staging/us-east-1` (l'arbre de
région live) · **Complément de :** le [guide maître d'infrastructure](README.md) et
le [guide GitOps](gitops-argocd.md).

Ceci est la référence par unité de l'arbre live Terragrunt : ce que chaque unité
provisionne, ce dont elle dépend, ce qu'elle produit en aval, et les invocations
exactes `plan`/`apply`/`destroy`. L'ordre ici est le **DAG d'apply** — les unités
consomment les sorties des unités qui les précèdent.

---

## 1. Structure & conventions

```
infrastructure/
├── modules/                    # reusable Terraform modules (the "how")
│   ├── networking/{vpc,route53}   eks   acm-cert   artifacts/ecr
│   ├── elasticache   msk   opensearch   s3-bucket (generic; Object-Lock param)
│   ├── kms-key   app-secrets   security/{irsa-roles,account-slr}   kubernetes/argocd
└── live/                       # Terragrunt instantiations (the "where/which")
    ├── global/{artifacts/ecr, networking/route53, security/ec2-spot-slr}  # account-shared
    ├── dev/us-east-1/…
    ├── staging/us-east-1/…     # ◄── documented here (the live path)
    └── prod/us-east-1/…        # full staging mirror, prod posture (not applied)
```

- **`root.hcl`** (parent) génère centralement le backend d'état distant S3 + lockfile
  et le bloc provider AWS pour chaque unité. Les unités individuelles ne les
  redéclarent jamais.
- **`region.hcl`** porte `aws_region`. Les unités le lisent via
  `read_terragrunt_config(find_in_parent_folders("region.hcl"))`.
- Les **blocs `dependency`** câblent la consommation de sorties inter-unités et
  *définissent le DAG*. La plupart des dépendances de datastore portent des
  `mock_outputs` limités à `["validate","plan"]`, si bien qu'un `run-all plan`
  fonctionne avant que les vraies ressources n'existent (bootstrap au premier run, ou
  après un démontage complet).
- **Un même module peut être à la base de plusieurs unités.** `s3-bucket` est à la
  base de trois unités différentes (`media-bucket`, `audit-worm`, `cnpg-backups`) avec
  des paramètres différents — Object-Lock activé pour l'audit, désactivé pour le média.

---

## 2. Le DAG d'apply (arbre de région staging)

Treize unités se résolvent dans cet ordre de dépendance. `terragrunt run-all apply`
parcourt le DAG automatiquement ; la numérotation montre les niveaux qui peuvent
s'exécuter en parallèle.

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

> **L'arête porteuse :** `security/irsa-roles` dépend des **ARN des datastores**
> (audit KMS/WORM, buckets media/cnpg), donc elle doit s'appliquer **après** les
> datastores — un réordonnancement par rapport à la disposition historique où IRSA
> venait plus tôt. Les `mock_outputs` sur ces dépendances permettent un `plan` à sec
> avant leur existence.
>
> **Pourquoi `acm-cert` est sa propre unité de niveau 0 :** elle a été découplée de
> `eks` (PR #543) afin que démonter le certificat ne puisse pas laisser une référence
> pendante qui ferait fuiter le VPC. Le cert est consommé par l'unité
> `kubernetes/argocd` (listener TLS du NLB), pas par `eks`.

---

## 3. Référence par unité

Légende : **Module** = module sous-jacent · **Depends on** = unités consommées ·
**Key outputs** = ce que les unités en aval lisent.

### Réseau & compute

| Unité | Module | Depends on | Provisionne / Key outputs |
|---|---|---|---|
| **`networking/vpc`** | `networking/vpc` | — | VPC, subnets publics/privés, CIDR, NAT. → `vpc_id`, IDs de subnets. Consommée par presque tout. |
| **`networking/acm-cert`** | `acm-cert` | — | Cert ACM public pour le listener WSS du NLB. → `certificate_arn`. Découplée de `eks` (#543). |
| **`eks`** | `eks` | `vpc` | Cluster EKS, **provider OIDC** (ancre de confiance IRSA), managed node groups (system + database). → `cluster_name`, `cluster_endpoint`, `cluster_certificate_authority_data`, OIDC issuer. |

### Plan de données (stores AWS managés)

| Unité | Module | Depends on | Provisionne / Key outputs |
|---|---|---|---|
| **`data/msk`** | `msk` | `vpc` | MSK (Kafka), SASL/SCRAM sur TLS + secret SCRAM. → `bootstrap_brokers_sasl_scram`. |
| **`data/elasticache`** | `elasticache` | `vpc` | ElastiCache Redis, cluster-mode + TLS + AUTH. → `configuration_endpoint`. |
| **`data/opensearch`** | `opensearch` | `vpc` | Domaine OpenSearch (VPC, TLS, fine-grained access) pour `search`. → `endpoint`. |
| **`data/media-bucket`** | `s3-bucket` | — | Bucket d'assets média : versionné, SSE-S3, CORS pour upload/download présigné. → ARN/nom du bucket. |
| **`data/audit-kms`** | `kms-key` | — | **KEK** d'audit (enveloppe les DEK par-sujet ; crypto-shred RGPD). → ARN de clé. Le seul principal est le rôle IRSA d'audit. |
| **`data/audit-worm`** | `s3-bucket` | `audit-kms` | Bucket de preuves de conformité : **Object-Lock COMPLIANCE** + SSE-KMS sous la KEK d'audit. → ARN du bucket. |
| **`data/cnpg-backups`** | `s3-bucket` | — | Cible de backup pour les clusters Postgres CNPG in-cluster. → ARN du bucket. |
| **`data/app-secrets`** | `app-secrets` | `media-bucket`, `audit-worm`, `audit-kms` | Seede/organise les entrées Secrets Manager que les ExternalSecrets de la flotte tirent. Ordonnancement seul pour l'unité argocd (`skip_outputs`). |

### Sécurité & livraison

| Unité | Module | Depends on | Provisionne / Key outputs |
|---|---|---|---|
| **`security/irsa-roles`** | `security/irsa-roles` | `eks`, `audit-kms`, `audit-worm`, `media-bucket`, `cnpg-backups` | Les rôles IRSA : ESO, Karpenter, LB controller, external-dns, cert-manager, EBS CSI, et les rôles applicatifs (audit=seul principal KMS/WORM, media=RW du bucket). → ARN par rôle. |
| **`kubernetes/argocd`** | `kubernetes/argocd` | `vpc`, `eks`, `security/irsa-roles`, `msk`, `elasticache`, `opensearch`, `acm-cert`, `app-secrets` | Installe ArgoCD + `root-bootstrap` (cible `bootstrap/staging`). Écrit **`cmp-envsubst-values`** (endpoints des datastores pour le CMP) et **`global-params-staging.json`**. Porte le **`before_hook` de graceful-cleanup sur `destroy`**. |

> L'unité `kubernetes/argocd` est la couture entre Terraform et GitOps : c'est la
> *dernière* unité Terraform et la *première* chose qui passe la main à ArgoCD (voir le
> [guide GitOps §4](gitops-argocd.md#4-bootstrap-order--terraform-then-gitops-converges)).

---

## 4. Référence des commandes

Exécutez depuis la racine de région sauf pour cibler une unité unique.

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

> **Fiez-vous au Run Summary, pas au code de sortie.** `terragrunt run-all` /
> `run --all` peut sortir en `0` même quand des unités individuelles rapportent
> `Failed`. Lisez le résumé `Succeeded / Failed` en fin de run.

### Le `GITHUB_TOKEN` à l'apply

L'unité `kubernetes/argocd` enregistre le dépôt auprès d'ArgoCD ; l'apply a besoin
d'un token GitHub dans l'environnement (`GITHUB_TOKEN=$(gh auth token)`). L'omettre
fait échouer l'étape d'enregistrement du dépôt ArgoCD alors que les ressources AWS
s'appliquent quand même — laissant un cluster à moitié bootstrappé.

---

## 5. Ordre de démontage & le hook de graceful-cleanup

`destroy` parcourt le DAG en **sens inverse**, si bien que `kubernetes/argocd` est
démontée en premier — ce qui est exactement là où le `before_hook "graceful_cleanup"`
se déclenche. Ce hook (`infrastructure/assets/teardown/k8s-graceful-cleanup.sh`)
draine les ressources AWS que les **contrôleurs in-cluster** ont créées et que
Terraform ne **possède pas** — ALB/NLB (et leurs ENI), volumes EBS de CNPG/Scylla, et
nœuds EC2 de Karpenter — **avant** que Terraform ne supprime le cluster et le VPC.
Sans lui, ces ressources fuient et les ENI de LB résiduelles bloquent le destroy de
`aws_vpc` avec `DependencyViolation`.

La boucle complète démontage → reconstruction, y compris les pièges d'état de
suppression Secrets-Manager/KMS qui survivent à un `destroy`, est documentée dans le
[runbook de cycle de vie d'un environnement](../runbooks/environment-lifecycle.md) et
le [runbook de reconstruction du staging jetable](../runbooks/staging-disposable-rebuild.md).

---

## 6. Écarts `dev` et `prod`

- **`dev`** — même ensemble de modules, mais les datastores sont **in-cluster**
  (Redpanda, StatefulSet ScyllaDB, Redis par-service, Postgres account) plutôt que
  des services AWS managés ; pas d'unités MSK/ElastiCache/OpenSearch/KMS/WORM. La
  livraison est le catalogue Helm legacy (`profile-service` seulement) plus
  `overlays/dev` pour l'itération locale.
- **`prod`** — un **miroir complet de l'arbre staging** (mêmes 13 unités) avec la
  posture de production activée : 3 AZ + NAT par AZ, groupes de nœuds Graviton
  (min 3, groupe `database` avec taint), MSK 3 brokers (`kafka.m5.large`, RF 3 /
  `min.insync.replicas` 2 par défaut du module), OpenSearch 3 nœuds réparti par
  zone, WORM d'audit en mode `COMPLIANCE`, et rien de jetable (pas de
  `force_destroy`, fenêtres de récupération des secrets conservées). ArgoCD suit
  **`main`** via `bootstrap/prod` + `global-params-prod.json`. **Pas encore
  appliqué** — les prérequis de mise en route (bucket d'état, CIDR du endpoint
  EKS, workflow de promotion d'images) sont documentés dans `live/prod/env.hcl`.

---

## Annexe — matrice module ↔ unité

| Module | Unités qui l'instancient |
|---|---|
| `networking/vpc` | `networking/vpc` |
| `acm-cert` | `networking/acm-cert` |
| `eks` | `eks` (staging, dev, prod) |
| `msk` / `elasticache` / `opensearch` | `data/msk` · `data/elasticache` · `data/opensearch` |
| `s3-bucket` (générique) | `data/media-bucket` (Lock off) · `data/audit-worm` (Lock : GOVERNANCE staging / COMPLIANCE prod) · `data/cnpg-backups` · `data/scylla-backups` |
| `kms-key` | `data/audit-kms` |
| `app-secrets` | `data/app-secrets` |
| `security/irsa-roles` | `security/irsa-roles` |
| `kubernetes/argocd` | `kubernetes/argocd` |
| `artifacts/ecr` | `global/artifacts/ecr` (partagé au compte) |
| `networking/route53` | `global/networking/route53` (partagé au compte) |
| `security/account-slr` | `global/security/ec2-spot-slr` (partagé au compte ; rôle lié au service EC2 Spot — global au compte, sûr à la destruction, autrefois par-env dans `irsa-roles`) |
