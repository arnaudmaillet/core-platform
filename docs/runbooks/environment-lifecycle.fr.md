---
i18n:
  source: ./environment-lifecycle.md
  source_sha256: 4d907ea03116574895f4b01eff4b299256e45cd838a5ed6ef562fb5c9e8eebd3
  translated_at: 2026-07-01
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`environment-lifecycle.md`](./environment-lifecycle.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, noms de topics, signatures, identifiants) sont volontairement
> laissés en anglais.

# Runbook : Cycle de vie d'un environnement (preflight → provisionnement → validation → démontage → reconstruction)

**Classe de document :** Runbook / Production · **Audience :** ingénieurs DevOps ·
**Environnement :** `staging` (le chemin GitOps live, jetable) · **Complément de :**
le [guide GitOps](../infrastructure/gitops-argocd.md), la
[référence des unités Terragrunt](../infrastructure/terragrunt-units.md), et le
runbook [reconstruction du staging jetable](staging-disposable-rebuild.md).

Staging est un **environnement jetable** : il est monté à partir de zéro, validé, et
démonté de façon répétée. Ce runbook est la boucle de bout en bout et les contraintes
d'ordonnancement qui maintiennent chaque phase sûre. Les deux dangers étroits et bien
connus — l'état de suppression Secrets-Manager/KMS et les fuites de load-balancers/ENI —
ont un outillage dédié que ce runbook déclenche au bon moment.

```
   ┌──────────┐   ┌───────────┐   ┌──────────┐   ┌────────────────────┐
   │ PREFLIGHT│──►│ PROVISION │──►│ VALIDATE │──►│ GRACEFUL TEARDOWN  │──┐
   └──────────┘   └───────────┘   └──────────┘   └────────────────────┘  │
        ▲                                                                 │
        └─────────────────────  REBUILD  ────────────────────────────────┘
```

Définissez ceci une fois par session :

```bash
BASE=infrastructure/live/staging/us-east-1
export AWS_REGION=us-east-1
```

---

## Phase 1 — Preflight (avant chaque apply)

Une reconstruction lancée trop tôt après un démontage entre en collision avec un état
AWS qui survit à `terragrunt destroy`. **Faites toujours le preflight**, même sur un
compte « propre ».

```bash
# Report-only: names still reserved by a prior teardown, orphan KMS keys
bash infrastructure/assets/teardown/preflight-clean-env.sh staging

# If anything is RESERVED, clear it (restore + force-delete to free the name)
bash infrastructure/assets/teardown/preflight-clean-env.sh staging --fix
```

**Ce qu'il vérifie et pourquoi (appris à la dure) :**

- Un secret Secrets Manager en `PendingDeletion` **réserve son nom pour toute la
  fenêtre de récupération**, et `describe-secret` renvoie `NotFound` pour lui — donc
  une vérification naïve « existe-t-il ? » signale le nom comme libre alors qu'il ne
  l'est pas. Le script utilise `list-secrets --include-planned-deletion` pour voir la
  vérité.
- Les modules fixent `recovery_window_in_days = 0` (PR #538) pour que les démontages
  suppriment les secrets immédiatement — mais AWS **réserve encore un nom
  force-supprimé pendant quelques minutes**.
- Les clés **KMS** ont une fenêtre de suppression minimale de 7 jours (pas de
  force-immédiat) ; une reconstruction crée des clés fraîches, donc les clés en attente
  sont un coût orphelin, pas un bloqueur — *sauf* si vous importez un secret périmé dont
  la clé est en attente (`KMSInvalidStateException`).

> **Cooldown :** si `--fix` a nettoyé quoi que ce soit, **attendez ~15 minutes** avant
> la Phase 2, ou `CreateSecret` entrera encore en collision sur le nom tout juste
> libéré. C'est l'échec de reconstruction le plus courant — respectez-le.

L'approfondissement des pièges d'état de suppression et l'alternative import/adopt vit
dans [staging-disposable-rebuild.md](staging-disposable-rebuild.md).

---

## Phase 2 — Provisionnement (Terraform, puis convergence GitOps)

Terraform monte la plateforme ; ArgoCD fait ensuite converger la flotte. Le DAG
d'apply et le détail par unité sont dans la
[référence des unités Terragrunt](../infrastructure/terragrunt-units.md) ; la checklist
de provisionnement complète (placeholders d'endpoints, ScyllaCluster, seeding des
secrets) est dans [`k8s/PROVISIONING-staging.md`](../../k8s/PROVISIONING-staging.md).
La séquence au niveau boucle :

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

**Contraintes d'ordonnancement qui doivent tenir (chacune est un vrai mode de
défaillance) :**

- `security/irsa-roles` s'applique **après** les datastores — elle consomme leurs ARN
  (audit KMS/WORM, buckets media/cnpg). Les `mock_outputs` permettent le `plan`
  antérieur.
- Les opérateurs au **wave −10** doivent être Healthy avant que la flotte de workloads
  (wave 0) n'applique ses CR `ScaledObject`/`Cluster`/`ExternalSecret` — sinon
  `no matches for kind`. Voir
  [GitOps §2](../infrastructure/gitops-argocd.md#2-sync-waves--the-load-bearing-ordering).
- L'unité `kubernetes/argocd` écrit le Secret **`cmp-envsubst-values`** ; sans lui la
  flotte rend des endpoints `${VAR}` littéraux. Relancez cette unité si les
  placeholders ne se résolvent pas.

> **Fiez-vous au Run Summary (`Succeeded / Failed`), PAS au code de sortie** —
> `terragrunt run --all` peut sortir en `0` avec des unités en échec.

---

## Phase 3 — Validation

Confirmez que la plateforme sert réellement avant de déclarer l'environnement monté.

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

**Dégradations connues et attendues (pas des échecs) :**

- Le plan WSS de `realtime` échoue en fail-closed (`RTM-1001`) jusqu'à ce que le JWKS
  de `auth` soit joignable — Keycloak est **DEFERRED**, donc c'est attendu. Le plan de
  santé gRPC n'est pas affecté, donc le pod devient quand même Ready.
- `AUTH_KEYCLOAK_CLIENT_SECRET` est un placeholder jusqu'à ce que Keycloak atterrisse.

Si un endpoint placeholder a fuité dans un pod (littéral
`${MSK_BOOTSTRAP_BROKERS_SASL_SCRAM}`), corrigez selon le
[mode de défaillance CMP GitOps](../infrastructure/gitops-argocd.md#34-cmp--envsubst-render-failures-staging-fleet-only).

---

## Phase 4 — Démontage gracieux

**Ne faites jamais `terraform destroy` d'un cluster live à l'aveugle.** Les contrôleurs
in-cluster (AWS LB controller, CNPG, scylla-operator, Karpenter) créent des ressources
AWS que Terraform ne **possède pas** ; un destroy aveugle les fait fuir, et les ENI de
load-balancer résiduelles bloquent le destroy de `aws_vpc` avec `DependencyViolation`.

L'unité `kubernetes/argocd` porte un **`before_hook "graceful_cleanup"` sur `destroy`**
qui exécute `infrastructure/assets/teardown/k8s-graceful-cleanup.sh` automatiquement —
vous lancez simplement le destroy normal :

```bash
( cd $BASE && terragrunt run --all destroy --non-interactive -- -auto-approve )
```

Parce que `destroy` parcourt le DAG en sens inverse, `kubernetes/argocd` est démontée en
premier, déclenchant le hook **avant** que `eks`/`vpc` ne soient touchés. Le hook, dans
l'ordre :

1. **Arrête le self-heal d'ArgoCD** (`patch app root-bootstrap … syncPolicy:null` ;
   supprime les appsets) pour qu'il ne puisse pas recréer ce qui est supprimé.
2. **Supprime les Ingress (ALB) et les Services `type=LoadBalancer` (NLB)**, puis
   **attend (~5 min) qu'AWS les déprovisionne réellement** — `kubectl delete svc`
   retourne avant que le LB controller ait supprimé le vrai NLB/les ENI. Cette attente
   est ce qui prévient la course au `ResourceInUseException` du cert ACM et la fuite
   d'ENI du VPC.
3. **Supprime les CR CNPG/Scylla puis les PVC**, pour qu'`ebs-csi` émette `DeleteVolume`
   (le `reclaimPolicy=Delete` ne se déclenche que sur une suppression ordonnée de PVC).
4. **Supprime les apps ArgoCD restantes sauf Karpenter.**
5. **Draine les NodeClaims/nœuds Karpenter pendant que Karpenter tourne encore**, pour
   qu'il termine les instances EC2 (et leurs ENI/EBS) via l'API EC2.

Chaque étape est best-effort (`|| true`) et idempotente — un cluster partiellement cassé
ne doit jamais bloquer le destroy.

### Si le démontage laisse un état périmé (la course ACM)

Si `eks` échoue à supprimer son cert ACM (`ResourceInUseException`) et que `vpc`
sort-tôt, les ressources AWS sont en général parties mais l'*état* de l'unité est
périmé. Réconciliez par unité, puis confirmez zéro fuite :

```bash
( cd $BASE/networking/acm-cert && terragrunt state list )   # inspect first
( cd $BASE/networking/vpc && terragrunt state rm $(cd $BASE/networking/vpc && terragrunt state list) )
aws ec2 describe-vpcs --filters Name=isDefault,Values=false  # expect none (ignore list-flicker; verify by --vpc-ids)
```

L'unité `acm-cert` découplée (PR #543) et l'attente de déprovisionnement des LB rendent
ceci rare.

---

## Phase 5 — Reconstruction

Une reconstruction n'est que la **Phase 1 → Phase 2 → Phase 3** à nouveau. La seule
préoccupation propre à la reconstruction est la dette d'état de suppression que la
Phase 1 nettoie. Si une unité de datastore échoue sur `already scheduled for deletion`
pendant la Phase 2, un nom est encore réservé :

- **Nettoyer (préféré) :** `preflight-clean-env.sh staging --fix`, attendre ~15 min,
  réappliquer.
- **Adopter (pas d'attente, mais MSK-risqué) :** restaurer + `terragrunt import` le
  secret — mais MSK heurte `KMSInvalidStateException` si la clé est en attente ;
  **préférez Nettoyer pour MSK**. Procédure complète dans
  [staging-disposable-rebuild.md](staging-disposable-rebuild.md).

---

## Référence rapide — toute la boucle

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

## Note sur la frontière

Tout dans ce runbook est **de la couche plateforme** — provisionnement, convergence
GitOps, et démontage cloud. Les développeurs d'application ne lancent jamais ces
commandes ; un service se livre en mergeant sur `develop` et en laissant ArgoCD
synchroniser (voir la
[section frontière du guide GitOps](../infrastructure/gitops-argocd.md#8-what-you-own-vs-what-the-platform-owns)).
