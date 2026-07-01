---
i18n:
  source: ./secrets-eso.md
  source_sha256: 7362e908bc9bd0eb75f1101022ed8029fc28f4802940d5fb5d93cb7f0eed1fb1
  translated_at: 2026-07-01
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`secrets-eso.md`](./secrets-eso.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, noms de topics, signatures, identifiants) sont volontairement
> laissés en anglais.

# Topologie des secrets — External Secrets Operator & ClusterSecretStore

**Classe de document :** Opérationnel / Production · **Audience :** ingénieurs DevOps
**et** auteurs de services (les deux traversent cette frontière) · **Périmètre :**
plan d'identifiants `staging` · **Complément de :** le
[guide maître d'infrastructure](README.md), le [guide GitOps](gitops-argocd.md), et
la [référence des unités Terragrunt](terragrunt-units.md).

Ce document est la source de vérité unique sur la façon dont un identifiant voyage
d'AWS jusqu'à l'environnement d'un pod, la distinction machine-généré-vs-seedé, et les
étapes exactes pour ajouter un nouveau secret. Si vous déboguez une variable d'env
manquante ou câblez un nouveau backend managé, commencez ici.

---

## 1. Le pipeline en une ligne

```
Terraform ──► AWS Secrets Manager ──► [ESO reads via IRSA] ──► k8s Secret ──► pod env (envFrom)
   (writes/seeds SM entries)   (ClusterSecretStore + ExternalSecret)     (deployment patch)
```

Rien dans le cluster ne parle jamais à AWS Secrets Manager, sauf l'**External Secrets
Operator (ESO)**. Les services lisent des Secrets Kubernetes ordinaires et ne
détiennent jamais d'identifiants AWS pour la récupération de secrets — c'est tout
l'intérêt de la topologie.

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

Manifestes : `k8s/overlays/staging/external-secrets.yaml`. L'intervalle de
rafraîchissement est de **1h** par ExternalSecret — une valeur SM rotée se propage au
Secret du pod en moins d'une heure (un redémarrage de pod la reprend immédiatement via
envFrom).

---

## 2. Pourquoi `ClusterSecretStore`, et non un `SecretStore` namespacé

C'est un choix délibéré et porteur (documenté en ligne dans le manifeste) :

- Le **ServiceAccount IRSA d'ESO vit dans le namespace `external-secrets`**.
- Un `SecretStore` namespacé ne peut référencer qu'un ServiceAccount **dans son propre
  namespace** — le webhook d'admission d'ESO rejette un `serviceAccountRef`
  cross-namespace (`"namespace should be empty or match the SecretStore's namespace"`).
- Un **`ClusterSecretStore` est cluster-scoped** et peut référencer le SA de
  l'opérateur dans `external-secrets` — le pattern IRSA canonique. Ainsi, les
  ExternalSecrets de la flotte (qui vivent dans le namespace de workload) pointent tous
  vers l'unique store cluster-scoped.

Le store s'authentifie avec `auth.jwt.serviceAccountRef` → le SA `external-secrets`,
dont le rôle IRSA (`enable_external_secrets` dans `modules/security/irsa-roles`)
accorde la lecture sur `core-platform-staging-*` (et le secret `AmazonMSK_*`).

> **Pas de sync-wave sur le store.** ESO **re-réconcilie** les ExternalSecrets une fois
> que le store existe, donc gater la flotte sur le store était inutile — et pire, un
> `sync-wave: -1` antérieur déployait **zéro pod** dès que le store échouait à
> s'appliquer. Laissez ESO converger plutôt que de l'ordonnancer.

---

## 3. Les deux classes d'identifiants

| Classe | Comment elle arrive dans Secrets Manager | Exemples |
|---|---|---|
| **Machine-généré** | Écrit par les **modules Terraform de datastore** au moment de l'apply. | MSK SCRAM (`AmazonMSK_…_app`), Redis AUTH (`…-redis-auth`), OpenSearch master (`…-opensearch-master`). |
| **Seedé** | Provisionné par l'unité Terragrunt **`data/app-secrets`** (auparavant créé hors-bande à la main). | `…-media-s3`, `…-audit-crypto`, `…-auth-secrets`. |

Les deux classes finissent en entrées SM sous le préfixe `core-platform-staging-*`
(ou `AmazonMSK_core-platform-staging_*`), et ESO les lit de manière uniforme. La
distinction ne compte qu'au moment du *provisionnement* : les valeurs machine-générées
apparaissent comme effet de bord de l'apply de l'unité de données ; les valeurs
seedées sont le travail de l'unité `data/app-secrets`. Voir la
[référence des unités Terragrunt](terragrunt-units.md#data-plane-managed-aws-stores).

---

## 4. Les cinq ExternalSecrets (ce que chacun alimente)

| ExternalSecret → k8s Secret | Consommé par | Clés (env var ← propriété SM) |
|---|---|---|
| **`backend-creds`** (`envFrom` de toute la flotte) | tout service touchant Kafka/Redis | `KAFKA_SASL_USERNAME/PASSWORD` ← secret app MSK ; `REDIS_PASSWORD` ← redis-auth |
| **`search-creds`** | `search` seulement | `SEARCH_OPENSEARCH_USER/PASSWORD` ← opensearch-master |
| **`media-s3-creds`** | `media` seulement | `MEDIA_S3_ACCESS_KEY/SECRET_KEY` ← media-s3 |
| **`audit-crypto`** | `audit` seulement | clés object+witness access/secret, `AUDIT_KEK_BASE64`, `AUDIT_CHECKPOINT_SIGNING_KEY_BASE64` ← audit-crypto |
| **`auth-secrets`** | `auth` seulement | `AUTH_SIGNING_PRIVATE/PUBLIC_PEM`, `AUTH_KEYCLOAK_CLIENT_SECRET` ← auth-secrets |

Deux conventions qui font trébucher :

- **Les noms de cibles sont littéraux** — Kustomize ne préfixe pas les champs *à
  l'intérieur* d'une CR, donc le `target.name` est écrit en toutes lettres (p. ex.
  `backend-creds`) pour correspondre exactement à l'`envFrom` du patch de déploiement.
  Le transformer `nameReference` de Kustomize n'atteint pas le corps des
  ExternalSecret ; l'entrée de configuration `external-secrets-refs-config.yaml` gère
  ce qu'il peut.
- **`secretKey` *est* le nom de la variable d'env** pour les secrets montés en
  envFrom — il doit correspondre à ce que le service lit dans le code, caractère pour
  caractère.

---

## 5. La nuance IRSA vs clés statiques (à lire avant de toucher media/audit)

**Tous les « rôles IRSA » ne sont pas réellement consommés par le code.** `media` et
`audit` construisent leurs clients S3/KMS avec des **identifiants statiques `rusty-s3`
SigV4**, *pas* avec la chaîne de credentials du SDK AWS — donc bien que les rôles IRSA
`media`/`audit` soient provisionnés, le code tel qu'il est ne les assume **pas** pour
l'accès à l'object-store. C'est pourquoi `media-s3-creds` et `audit-crypto` portent des
**clés d'accès IAM-user statiques** seedées via `data/app-secrets`.

- Les rôles IRSA restent la cible correcte *si le code migre vers le SDK* — c'est un
  report suivi, pas un bug.
- ESO lui-même **utilise** IRSA (son SA assume le rôle `external_secrets`). La
  distinction est : ESO utilise IRSA pour *lire les secrets* ; media/audit utilisent
  des *clés statiques issues de ces secrets* pour atteindre S3.

Pour `audit`, la vraie custody AWS KMS/HSM et un véritable témoin WORM cross-account
sont le report externe documenté ; le câblage ici est le **chemin v1 ENV-KEK**
(`AUDIT_KEK_BASE64`) avec le témoin pointé vers le même bucket WORM.

---

## 6. Ajouter un nouveau secret (la procédure exacte)

Cela traverse la frontière plateforme/application — trois modifications ordonnées,
une par couche :

**A. Provisionner l'entrée SM (plateforme).**
- *Machine-généré ?* Elle apparaît quand le module de données s'applique — rien à ajouter.
- *Seedé ?* Ajoutez-le à l'unité **`data/app-secrets`** pour que Terraform l'écrive sous
  `core-platform-staging-<name>`. Ne faites jamais `aws secretsmanager create-secret` à
  la main pour un secret permanent — il ne survivra pas à une reconstruction.

**B. Le projeter dans le cluster (plateforme).** Ajoutez un `ExternalSecret` à
`k8s/overlays/staging/external-secrets.yaml` :

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

La policy de lecture d'ESO couvre déjà `core-platform-staging-*`, donc **aucun
changement IAM** n'est nécessaire pour un secret sous ce préfixe. Un nouveau préfixe
nécessite d'élargir la policy du rôle `external_secrets` dans
`modules/security/irsa-roles`.

**C. Le consommer (application).** Ajoutez un patch `envFrom` au déploiement du service
pointant vers le Secret `my-new-creds`, et lisez `MY_SERVICE_TOKEN` depuis
l'environnement dans le code. C'est la seule étape qu'un auteur de service possède.

Puis validez le rendu et laissez GitOps converger :

```bash
kubectl kustomize k8s/overlays/staging | grep -A3 my-new-creds   # renders?
# merge to develop → ArgoCD syncs → ESO materializes the Secret
```

---

## 7. Opérer & déboguer

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

### Modes de défaillance

| Symptôme | Cause | Correctif |
|---|---|---|
| `ExternalSecret` `SecretSyncedError`, `AccessDenied` | Le rôle IRSA d'ESO ne peut pas lire la clé SM (mauvais préfixe / policy) | Confirmez que la clé est sous `core-platform-staging-*` ; sinon élargissez la policy `external_secrets`. |
| `SecretSyncedError`, `ResourceNotFoundException` | L'entrée SM n'existe pas (secret seedé jamais provisionné) | Appliquez `data/app-secrets` ; vérifiez que les noms de propriété correspondent. |
| `SecretSyncedError`, `PendingDeletion` après une reconstruction | Nom SM réservé par un démontage précédent | Lancez `preflight-clean-env.sh staging --fix`, attendez, réappliquez — voir le [runbook de cycle de vie](../runbooks/environment-lifecycle.md). |
| L'env du pod a la variable mais valeur erronée/vide | Nom de `property` incorrect, ou valeur SM seedée à vide (p. ex. placeholder Keycloak) | Corrigez le `remoteRef.property` ; notez que `AUTH_KEYCLOAK_CLIENT_SECRET` est un **placeholder** jusqu'à Keycloak (DEFERRED). |
| Store `ValidationFailed`, `serviceAccountRef` rejeté | Quelqu'un l'a changé en `SecretStore` namespacé | Ce doit être un `ClusterSecretStore` (§2). |

---

## 8. Résumé de la frontière

- **La plateforme possède :** les entrées Secrets Manager (Terraform), le rôle IRSA
  d'ESO, le `ClusterSecretStore`, et les définitions d'`ExternalSecret`.
- **Les auteurs de services possèdent :** le patch `envFrom` et la lecture de la
  variable d'env dans le code — et le choix du nom de la variable d'env, qui doit
  égaler le `secretKey`.
- **Le contrat entre eux** est le nom du Secret k8s + ses clés. Accordez-vous là-dessus,
  et aucun des deux côtés n'a besoin des rouages de l'autre.
