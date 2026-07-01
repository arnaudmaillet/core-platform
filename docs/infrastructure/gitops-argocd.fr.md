---
i18n:
  source: ./gitops-argocd.md
  source_sha256: ca3827740be5672df6ec64d7ef392865f80b93e10d4f6300f16924cb23fa6fde
  translated_at: 2026-07-01
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`gitops-argocd.md`](./gitops-argocd.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, noms de topics, signatures, identifiants) sont volontairement
> laissés en anglais.

# Guide des opérations GitOps & ArgoCD

**Classe de document :** Opérationnel / Production · **Audience :** ingénieurs
DevOps & plateforme (développeurs d'application : voir la section
[frontière](#8-ce-que-vous-possédez-vs-ce-que-possède-la-plateforme)) ·
**Environnement :** `staging` (le chemin GitOps live), avec les écarts `dev`/`prod`
en ligne · **Complément de :** le [guide maître d'infrastructure](README.md).

Ce guide est le manuel opérationnel du plan de livraison : comment les manifestes
dans Git deviennent des workloads en exécution, dans quel ordre, comment ArgoCD les
y maintient, et exactement quoi lancer quand une synchronisation échoue.

---

## 1. Le modèle de livraison en une image

ArgoCD exécute des **App-of-AppSets** : une unique Application `root-bootstrap`
(installée par l'unité Terragrunt `kubernetes/argocd`) pointe vers un dossier de
bootstrap propre à l'environnement ; ce dossier contient des ApplicationSets qui se
déploient en éventail vers les Applications d'infra individuelles et la flotte de
workloads.

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

- **Racine de confiance :** Terraform n'installe qu'une seule chose dans le cluster —
  `root-bootstrap`. Tout le reste est piloté depuis Git à partir de là. C'est
  pourquoi l'ordre de bootstrap (§4) place Terraform *avant* tout workload.
- **`develop` est la révision suivie.** Chaque AppSet et Application fixe
  `targetRevision: develop`. ArgoCD réconcilie le cluster vers `develop` avec
  `selfHeal: true`. **`develop` est protégée — ne poussez jamais dessus directement ;
  créez une branche, une PR, mergez, et laissez ArgoCD converger.**
- **Dépôt :** `https://github.com/arnaudmaillet/core-platform` pour toutes les sources.

---

## 2. Sync waves — l'ordonnancement porteur

Les sync waves sont des annotations (`argocd.argoproj.io/sync-wave`) qu'ArgoCD honore
du plus petit au plus grand. Cet ordre n'est **pas cosmétique** : les CRD d'un
opérateur doivent être établies avant qu'un workload n'applique une ressource
personnalisée qui les référence, sinon la synchronisation du workload échoue avec
`no matches for kind`.

| Wave | AppSet | Contenu | Pourquoi il doit passer en premier |
|---:|---|---|---|
| **−10** | `root-operators` | CNPG operator, External Secrets, **KEDA**, scylla-operator, k6-operator | Installe les CRD (`Cluster`, `ExternalSecret`, `ScaledObject`, `ScyllaCluster`) dont la flotte dépend. |
| **−5** | `root-security` | cert-manager (+config), external-dns, admin-access | Les émetteurs TLS et le DNS doivent exister avant que les objets Ingress/Service ne demandent certs/enregistrements. |
| **−5** | `root-platform` | Karpenter (+config), aws-lb-controller, metrics-server, storage-class | L'autoscaling compute, le contrôleur LB (provisionne NLB/ALB), la **StorageClass gp3**, et la source de métriques du HPA. |
| **−5** | `root-observability` | monitoring | Cibles de scrape et tableaux de bord. |
| **0** | `root-workloads` | `staging-fleet` Application → `k8s/overlays/staging` | Les services eux-mêmes — ils déclarent des CR `ScaledObject`/`Cluster`/`ExternalSecret`, donc ils doivent passer en dernier. |

> **Mode de défaillance — course aux CRD.** Si un workload App reste bloqué en
> `SyncFailed` avec `unable to recognize "...": no matches for kind "ScaledObject"`,
> un opérateur au wave −10 n'a pas terminé. Ne **relancez pas** le workload ; corrigez
> d'abord l'opérateur (§7.2), puis la synchronisation du workload réussit sans changement.

### La politique de prune diffère par tier, à dessein

- Les **AppSets d'infra** (`root-operators`, etc.) utilisent `prune: true` — la
  plateforme est entièrement déclarative ; tout ce qui n'est pas dans Git doit être
  supprimé.
- **`root-workloads` (legacy dev)** utilise `prune: false` — un garde-fou contre une
  suppression massive accidentelle des services en exécution suite à un mauvais générateur.
- **`staging-fleet`** utilise `prune: true` + `selfHeal: true` — staging est la
  flotte managée et jetable et doit correspondre exactement à Git.

---

## 3. Le Config Management Plugin envsubst (pourquoi `source.plugin`, pas `path`)

L'overlay staging porte des **endpoints d'exécution qui ne sont connus qu'après
l'apply Terraform des datastores** — les brokers MSK, les endpoints
ElastiCache/OpenSearch, l'ARN du certificat ACM. Ils vivent dans les manifestes sous
forme de placeholders `${VAR}`.

Ils sont résolus par un **Config Management Plugin `envsubst`** exécuté en sidecar
dans `argocd-repo-server` (défini dans `modules/kubernetes/argocd`). Le plugin exécute :

```
kustomize build k8s/overlays/staging | envsubst
```

sur un **Secret de valeurs détenu par Terraform** (`cmp-envsubst-values`). L'unité
Terragrunt `kubernetes/argocd` écrit ce Secret à partir des sorties des datastores
(`msk_bootstrap_brokers`, `elasticache_endpoint`, `opensearch_endpoint`,
`ssl_certificate_arn`). D'où le fait que l'Application `staging-fleet` référence
`source.plugin.name: envsubst-v1.0`, et **non** un `path` simple.

```
Terraform data-store outputs ──► kubernetes/argocd unit ──► cmp-envsubst-values Secret
                                                                     │
  Git: k8s/overlays/staging (${VAR} templates) ── repo-server CMP ───┘
        │
        └─ kustomize build | envsubst ──► concrete manifests ──► cluster
```

**Pourquoi ce design et pas des valeurs concrètes commitées :** Git contient le
*template* ; le CMP rend les manifestes concrets au moment de la synchronisation.
Parce que la sortie rendue est déterministe à partir de Git + le Secret,
**`selfHeal` est stable** — il n'y a aucune modification manuelle dans le cluster
qu'ArgoCD aurait à combattre. Le plugin est nommé `<metadata.name>-<spec.version>`,
donc le plugin défini comme `envsubst` / `v1.0` est référencé **`envsubst-v1.0`**.

> **Mode de défaillance — placeholder non résolu.** Un env de pod affichant un
> `${MSK_BOOTSTRAP_BROKERS_SASL_SCRAM}` littéral signifie que le CMP s'est exécuté
> mais que le Secret n'avait pas la clé. La valeur appartient à Terraform : relancez
> l'unité `kubernetes/argocd` pour qu'elle réécrive `cmp-envsubst-values`, puis
> forcez un hard-refresh de l'App (§6). Placeholders laissés vides à dessein jusqu'à
> ce qu'une dépendance atterrisse : `AUTH_JWKS_URL`, `KEYCLOAK_TOKEN_ENDPOINT`
> (DEFERRED jusqu'à Keycloak).

---

## 4. Ordre de bootstrap — Terraform, puis convergence GitOps

Le plan de livraison ne peut pas démarrer avant que la plateforme sur laquelle il
tourne existe. La séquence complète (détail des datastores dans la
[référence des unités Terragrunt](terragrunt-units.md)) :

```
1. Terragrunt run-all apply           # vpc → eks → data/* → security/irsa-roles → kubernetes/argocd
                                       #   (the argocd unit installs ArgoCD + root-bootstrap
                                       #    and writes cmp-envsubst-values + global-params-staging.json)
2. GitOps: operators converge         # wave -10 — CNPG, scylla-operator, ESO, KEDA come Healthy
3. kubectl apply -k k8s/base/infra/scylla-cluster   # the ScyllaCluster CR (un-prefixed FQDN)
4. GitOps: security/platform/observability   # wave -5
5. GitOps: staging-fleet syncs         # wave 0 — the services
```

Suivre la convergence :

```bash
# after the Terragrunt apply completes
aws eks update-kubeconfig --name <cluster> --region us-east-1
kubectl -n argocd get applications -w        # wait for root-* then staging-fleet to be Synced/Healthy
```

La séquence de bout en bout, annotée et faisant autorité, vit dans
[`runbooks/audit-remediation-rollout.md`](../runbooks/audit-remediation-rollout.md) ;
la boucle jetable complète (y compris le démontage) est dans
[`runbooks/environment-lifecycle.md`](../runbooks/environment-lifecycle.md).

---

## 5. Le catalogue d'apps d'infra (ce qu'est chaque App)

`infrastructure/argocd/apps/infrastructure/` — regroupées par l'AppSet qui les
déploie en éventail. Chacune est une App Helm/Kustomize légère superposée à
`global-params-staging.json`.

| Groupe (wave) | App | Rôle |
|---|---|---|
| **operators (−10)** | `cnpg-operator` | CloudNativePG — les 6 clusters Postgres in-cluster. |
| | `external-secrets` | ESO — projette AWS Secrets Manager dans des Secrets k8s (voir la [topologie des secrets](secrets-eso.md)). |
| | `keda` | Autoscaling sur le lag Kafka pour les stream workers. |
| | `scylla-operator` | Gère le ScyllaCluster. |
| | `k6-operator` | Orchestration des tests de charge. |
| **security (−5)** | `cert-manager` / `-config` | Émission TLS ; la config câble le(s) émetteur(s). |
| | `external-dns` | Enregistrements Route53 pour Services/Ingress. |
| | `admin-access` | RBAC / bindings admin. |
| **platform (−5)** | `karpenter` / `-config` | Contrôleur d'autoscaling des nœuds + NodePools/EC2NodeClasses. |
| | `aws-lb-controller` | Provisionne le NLB (WSS realtime) / les ALB. |
| | `metrics-server` | Alimente le HPA. |
| | `storage-class` | La StorageClass **gp3** par défaut (le plan de stockage). |
| **observability (−5)** | `monitoring` | Métriques/tableaux de bord. |

Chaque App reçoit `global-params-staging.json` (ID de compte, région, nom du
cluster, ARN des rôles IAM d'addon) via une ref Helm `$values` ou le fichier de
params, si bien que les mêmes définitions d'App se rendent par environnement sans duplication.

---

## 6. Opérations au quotidien (invocations exactes)

Toutes les commandes supposent que `kubectl` pointe vers le cluster cible et
qu'ArgoCD est dans le namespace `argocd`. Préférez l'API/UI ArgoCD pour la sync ;
`kubectl` est la trappe de secours.

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

### Suspendre la réconciliation (maintenance / incident)

`selfHeal` annulera un `kubectl edit` manuel en quelques secondes. Pour qu'une
modification hors-bande délibérée persiste, **suspendez** d'abord l'App :

```bash
# Pause auto-sync so a manual change survives
kubectl -n argocd patch application staging-fleet --type merge \
  -p '{"spec":{"syncPolicy":{"automated":null}}}'
# ... do the manual thing ...
# Re-enable (let Git win again)
kubectl -n argocd patch application staging-fleet --type merge \
  -p '{"spec":{"syncPolicy":{"automated":{"prune":true,"selfHeal":true}}}}'
```

> Le hook de démontage utilise le même levier à la racine :
> `kubectl patch app root-bootstrap -n argocd --type merge -p '{"spec":{"syncPolicy":null}}'`
> pour empêcher ArgoCD de recréer ce qu'un destroy supprime.

---

## 7. Modes de défaillance & récupération

### 7.1 App bloquée en `OutOfSync` / `Progressing` indéfiniment

```bash
argocd app get <app>                       # read the message on each resource
kubectl -n argocd logs deploy/argocd-application-controller | tail -50
```

- **Conflit de champ immuable** (p. ex. un changement de `selector` de Deployment) :
  ArgoCD ne peut pas le patcher. Supprimez la ressource fautive et laissez la sync la
  recréer (`argocd app sync <app> --resource <group:kind:name> --force`).
- **Conflit de field manager ServerSideApply :** toutes les Apps utilisent
  `ServerSideApply=true` ; un manager en conflit nécessite `--force` sur la sync.

### 7.2 La sync d'un workload échoue avec `no matches for kind "ScaledObject"/"Cluster"`

L'opérateur (wave −10) n'a pas établi sa CRD. Vérifiez et soignez l'opérateur, pas le
workload :

```bash
kubectl -n argocd get application keda cnpg-operator scylla-operator external-secrets
kubectl get crd | grep -E 'scaledobjects|clusters.postgresql|scyllaclusters|externalsecrets'
argocd app sync keda                       # re-drive the operator, then the workload converges on its own
```

### 7.3 `selfHeal` combat une modification manuelle (le changement est sans cesse annulé)

Comportement attendu — Git est la source de vérité. Soit commitez le changement sur
`develop` (le bon chemin), soit suspendez l'automatisation (§6) pour un contournement
temporaire délibéré. Ne désactivez jamais `selfHeal` dans Git pour gagner une dispute
avec lui.

### 7.4 Échecs de rendu CMP / envsubst (`staging-fleet` uniquement)

```bash
kubectl -n argocd logs deploy/argocd-repo-server -c envsubst | tail -50   # the sidecar
kubectl -n argocd get secret cmp-envsubst-values -o yaml                   # the Terraform-owned values
```

- Clé manquante/vide → relancez l'unité Terragrunt `kubernetes/argocd` (elle détient
  le Secret), puis `argocd app get staging-fleet --hard-refresh`.
- Une erreur de `kustomize build` → validez localement d'abord :
  `kubectl kustomize k8s/overlays/staging`.

### 7.5 Lacunes du transformer de référence de CRD (préfixe silencieusement erroné)

Le transformer `nameReference` intégré de Kustomize ne connaît **pas**
`ScaledObject.scaleTargetRef` de KEDA ni `ScheduledBackup.spec.cluster.name` de CNPG.
Les overlays ajoutent des entrées `configurations:` (`*-refs-config.yaml`) pour que le
`namePrefix` se propage. Symptôme d'une entrée manquante : un scaler/backup préfixé qui
cible une ressource inexistante (non préfixée) — l'App est `Healthy` mais rien ne
s'échelonne/se sauvegarde. Quand vous introduisez une CRD qui référence une autre
ressource par nom, ajoutez l'entrée de config (voir le guide maître §1.4 et les
Conventions dans `CLAUDE.md`).

---

## 8. Ce que vous possédez vs ce que possède la plateforme

**Développeurs d'application — vous interagissez avec GitOps à exactement trois
coutures ; vous n'opérez pas ArgoCD :**

1. **Vos manifestes** vivent dans `k8s/base/services/<svc>` et sont superposés par les
   overlays. Merger sur `develop` est votre déclencheur de déploiement — ArgoCD fait
   le reste.
2. **Votre tag d'image.** L'overlay staging est épinglé sur des tags immuables
   `:<git-sha>` par le job CI de la flotte. Ne réintroduisez pas un tag flottant
   `:staging` — ArgoCD ne redéploiera pas sur un re-push de tag mutable sans un bump de
   digest.
3. **Votre config/secrets** arrivent en env depuis les placeholders `${VAR}` (endpoints)
   et des Secrets montés (identifiants). Pour en ajouter un, suivez le
   [guide de topologie des secrets](secrets-eso.md) — vous ne touchez pas à ArgoCD.

**Les ingénieurs plateforme possèdent :** les AppSets, les sync waves, le CMP, le
catalogue d'apps d'infra, et la protection de `develop`. Tout ce qui est en §§1–7 est à vous.

---

## Annexe — référence rapide

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
