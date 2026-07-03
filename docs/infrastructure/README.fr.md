---
i18n:
  source: ./README.md
  source_sha256: f697ecfd8c02a6d32eb5b487c9e05315a41fca99681d32e86901105c0ed423c6
  translated_at: 2026-07-03
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, noms de topics, signatures, identifiants) sont volontairement
> laissés en anglais.

# Core-Platform — Documentation d'Infrastructure et d'Exploitation

**Classe du document :** Définitive / Qualité production · **Périmètre :** monorepo `core-platform` (services, IaC Terragrunt, livraison Kustomize, topologie événementielle) · **Environnement principal documenté :** `staging` (le chemin Kustomize/GitOps actif), avec mention des écarts `dev` et `prod` · **Statut :** Rédigé à partir de l'état actuel de `develop` ; l'infrastructure de staging est rédigée et validée statiquement mais pas encore appliquée à un cluster réel.

> **Ceci est la référence canonique.** Pour les approfondissements opérationnels orientés tâche, voir :
> - [Opérations GitOps & ArgoCD](gitops-argocd.md) — la cascade App-of-AppSets, les sync waves, le CMP envsubst, et les opérations ArgoCD au quotidien.
> - [Référence des unités Terragrunt](terragrunt-units.md) — entrées/sorties/dépendances par unité, le DAG d'apply, et les invocations exactes.
> - [Topologie des secrets (ESO / ClusterSecretStore)](secrets-eso.md) — comment un identifiant voyage de Terraform → Secrets Manager → env du pod.
> - [Runbook de cycle de vie d'un environnement](../runbooks/environment-lifecycle.md) — preflight → provisionnement → validation → démontage gracieux → reconstruction.
> - [Point d'entrée & taxonomie de la documentation](../README.md) — le routeur d'audience et la frontière plateforme/application.

---

## 1. Architecture Technique Fondamentale et Archétypes de Déploiement

### 1.1 Modèle de composition

La plateforme est un unique espace de travail Rust (`crates/`) compilé en **images de conteneur par binaire** via un `deploy/Dockerfile` générique et optimisé pour le cache (`--build-arg BIN=<package>`). Chaque service est un crate hexagonal orienté domaine (DDD : `domain → application(ports) → infrastructure(adaptateurs)`), exposé par un ou plusieurs binaires déployables. Les préoccupations transverses sont des crates de fondation partagés (`service-runtime`, `transport` (Kafka + gRPC), `cqrs`, adaptateurs de stockage Postgres/Scylla/Redis, `auth-context`, `telemetry`).

Deux plans de contrats régissent l'intégration, tous deux contrôlés à la compilation :

- **Synchrone (gRPC) :** crates proto versionnés `*-api`, protégés par `buf breaking`.
- **Asynchrone (Kafka) :** le crate de registre `event-topology` — la source unique de vérité du câblage producteur/consommateur, doté d'un test de contrat qui fait échouer la compilation sur une *arête fantôme* (un consommateur d'un topic qu'aucun producteur n'émet). C'est la cartographie de référence du tissu de streaming (§2.4).

### 1.2 Niveaux de service et sémantique de défaillance

Le niveau (« tier ») est un contrat d'exécution explicite (label de pod `tier:`) qui dicte la posture en cas de défaillance :

| Niveau | Posture | Services | Signification |
|---|---|---|---|
| **TIER-0** | **Fail-closed** | `auth` (50060), `moderation` (50061), `audit-server` (50068), `audit-worker` (50069) | Identité, confiance/sécurité, conformité infalsifiable. Exactitude prioritaire sur disponibilité — p. ex. audit refuse une écriture privilégiée non enregistrable (« break-glass »). |
| **TIER-1** | **Fail-open** | `counter-server/worker` (50064/50065), `media` (50063), `search` (50062), `realtime-gateway` (8443/50066), `realtime-dispatcher` (50067) | Systèmes-de-Référence / -de-Connexion / -de-Livraison. Disponibilité prioritaire — dégradation gracieuse, re-dérivation depuis les SoR amont. |
| **Cœur (implicite)** | Mixte | `account` (50059), `profile` (50052), `social-graph` (50053), `post` (50056), `comment` (50057), `engagement` (50058), `geo-discovery` (50054), `notification` (50055), `timeline` (50070), `chat` (50051) | Les Systèmes-d'Enregistrement du graphe social et les modèles de lecture. |

Les ports internes sont des ClusterIP par service ; chaque service possède désormais un port distinct (`timeline` déplacé de 50060 → 50070 pour lever sa réutilisation du port d'`auth`).

### 1.3 Archétypes de déploiement

La flotte se résout en **quatre** archétypes réutilisables, distingués par caractéristique d'exécution plutôt que par domaine :

1. **Serveur RPC** — lié aux requêtes, gRPC, mis à l'échelle sur le CPU. La disponibilité (« readiness ») dépend du health gRPC typé (`SERVING` uniquement après succès des sondes backend) ; la vivacité (« liveness ») ne vérifie que le processus, de sorte qu'une interruption backend transitoire retire le pod de la rotation sans boucle de redémarrage. *(tous les binaires `*-server`)*
2. **Worker de flux** — lié à la consommation Kafka, sans RPC métier (plan health/reflection uniquement), mis à l'échelle sur le **retard du groupe de consommateurs**. *(`counter-worker`, `audit-worker`, `realtime-dispatcher`)*
3. **Périphérie à état** — `realtime-gateway` : détient une table de connexions longue durée (un socket en « future parquée » par appareil, conception C10M), mis à l'échelle sur connexions/mémoire, protégé par un **PodDisruptionBudget** pour des drainages progressifs, exposé publiquement via un **NLB L4** (jamais un ALB — cf. §2.5).
4. **Init-container de migration** — chaque service adossé à Postgres/Scylla exécute le `migrator` partagé en init-container idempotent (`args: [<service>]`) avant le conteneur d'exécution ; les services bi-stores (`counter`, `moderation`) en exécutent un par backend.

### 1.4 Modèle de mise à l'échelle

| Mécanisme | Déclencheur | Charges de travail |
|---|---|---|
| **HPA** | CPU | `auth`, `moderation`, `counter-server`, `media`, `search`, `realtime-gateway`, `audit-server` |
| **KEDA ScaledObject** | Retard du groupe de consommateurs Kafka | `counter-worker` (max 12), `realtime-dispatcher` (max 8), `audit-worker` (max 4) |
| **PDB** | Plancher en cas de perturbation volontaire | `realtime-gateway` (minAvailable 1) |

KEDA est un prérequis ferme (opérateur en sync-wave GitOps −10). Une extension `nameReference` de Kustomize propage le `namePrefix` d'environnement dans `ScaledObject.scaleTargetRef` et `TriggerAuthentication.authenticationRef` (le transformateur intégré couvre l'HPA mais pas les CRD `keda.sh`) — sans elle, un scaler préfixé cible silencieusement un Deployment inexistant. **Le maxReplicaCount d'un scaler sur retard ne doit jamais excéder le nombre de partitions du topic** (un groupe de consommateurs ne peut pas se paralléliser au-delà de ses partitions).

---

## 2. État de l'Infrastructure AWS et du Dépôt

### 2.1 Topologie des environnements

| Env | Backends | Chemin de livraison | État |
|---|---|---|---|
| **dev** | En cluster (Redpanda, StatefulSet ScyllaDB, Redis par service, Postgres account) | Catalogue Helm ArgoCD historique (`apps/catalog`), `profile` seul actif ; Kustomize `overlays/dev` en local | Partiel |
| **staging** | **AWS managé** (MSK, ElastiCache, OpenSearch, S3, KMS) + opérateurs en cluster (scylla-operator, CNPG) | **Application ArgoCD `staging-fleet` → `k8s/overlays/staging`** (Kustomize) | Rédigé, validé, **non appliqué** |
| **prod** | **Miroir AWS managé de staging** avec la posture de production (3 AZ + NAT par AZ, MSK 3 brokers, WORM COMPLIANCE, rien de jetable) | **Application ArgoCD `prod-fleet` → `k8s/overlays/prod`**, qui suit **`main`** (merger develop → main est le déploiement prod) | Échafaudé, **non appliqué** (prérequis : `live/prod/env.hcl`) |

Tous les environnements ciblent le compte AWS `724772065879` / `us-east-1`, partageant un unique registre ECR (un dépôt par binaire, étiqueté par environnement).

### 2.2 Modules Terraform et structure Terragrunt

**Modules** (`infrastructure/modules/`) : `networking/{vpc,route53}`, `eks`, `artifacts/ecr`, `security/irsa-roles`, `kubernetes/argocd`, `elasticache`, `msk`, `opensearch`, `s3-bucket` (générique ; paramètre Object-Lock), `kms-key`.

**Arbre Terragrunt live** (`infrastructure/live/<env>/us-east-1/`) : `networking/vpc → eks → data/{msk,elasticache,opensearch,media-bucket,audit-kms,audit-worm} → security/irsa-roles → kubernetes/argocd`. L'état distant (S3 + fichier de verrou) et les providers sont générés centralement par `root.hcl`. `global/artifacts/ecr` est la liste de registre faisant autorité, partagée au niveau du compte (tous les binaires de la flotte + `migrator` + `buildcache`).

### 2.3 Magasins de données managés (staging)

| Magasin | Module | Rôle | Chemin des identifiants |
|---|---|---|---|
| **MSK (Kafka)** | `msk` | Tissu d'événements asynchrone ; SASL/SCRAM sur TLS | Secret SCRAM → Secrets Manager → ESO → `backend-creds` |
| **ElastiCache (Redis)** | `elasticache` | Niveaux chauds, présence/registre, diffusion ; mode cluster + TLS + AUTH | Jeton AUTH → SM → ESO → `backend-creds` |
| **OpenSearch** | `opensearch` | Index inversé de `search` (Système-de-Référence) ; VPC, TLS, accès fin | Utilisateur maître → SM → ESO → `search-creds` |
| **S3 — media** | `s3-bucket` | Octets des actifs ; versionné, SSE-S3, CORS pour téléversement/téléchargement présignés | Clés statiques → SM → ESO → `media-s3-creds` |
| **S3 — audit WORM** | `s3-bucket` | Ancre de preuve de conformité ; **Object-Lock COMPLIANCE** + SSE-KMS | Clés statiques + KEK → SM → ESO → `audit-crypto` |
| **KMS** | `kms-key` | KEK d'audit (enveloppe les DEK par sujet ; crypto-effacement RGPD) | Restreint par IRSA (principal unique) |
| **CNPG Postgres ×6** | en cluster (overlay) | `account`, `counter` (registre tiède), `audit` (chaîne), `moderation`, `auth`, `media` | Secret `<name>-app` généré par le cluster (`uri`) |
| **ScyllaDB** | scylla-operator | `counter` (froid TWCS), `moderation` (historique) + keyspaces du graphe social | En cluster, sans authentification |

### 2.4 Topologie des pipelines asynchrones

Extraite du registre `event-topology` (la source de vérité imposée à la compilation). Flux principaux :

- **Ingestion de conformité (TIER-0) :** `account.v1.events`, `auth.v1.events`, `moderation.v1.events` → **`audit`**. Audit est un puits terminal ; il auto-consomme aussi une voie d'ingestion générique `audit.v1.events` alimentée par le chemin gRPC synchrone `RecordPrivileged`.
- **Modèle de lecture de découverte :** `profile.v1.events` + `post.v1.events` + `moderation.v1.events` → **`search`** (index + visibilité).
- **Engagement → magnitudes :** `engagement.reactions` → **`counter`** (agrégation), `notification`, soi-même (write-behind). Counter consomme aussi de la télémétrie amont différée (`view/impression/click.v1.events`).
- **Counter → temps réel + viralité :** `counter.v1.popularity` → **`realtime`** (diffusion) + **`geo-discovery`** (re-scoring).
- **Diffusion sociale :** `social-graph.followed/unfollowed` → **`timeline`** ; `social-graph.author_tier_changed` → **`profile`** (propriété du niveau).
- **Push temps réel :** `post.v1.events` → **`realtime`** ; `media.v1.events` auto-consommé (transformation Plan-B) ; `moderation.v1.events` → **`media`** (retrait).

Le registre suit aussi formellement les consommateurs **DIFFÉRÉS** (producteurs externes/non construits : `moderation.reports/signals`, `view/impression/click.v1.events`, le décalage de nommage `social-graph.follows`) et les **PRODUCTEURS ORPHELINS** (marge intentionnelle : `post.updated` historique, `social-graph.blocked` imposé sur le chemin de lecture, les topics du plan de livraison de chat). À noter : le registre garantit le **câblage** des topics, pas la **forme** des charges utiles — un écart connu de charge utile `post → geo/notification` demeure une préoccupation distincte et suivie.

Le registre est aussi la **source de provisionnement des brokers** : le binaire `topic-provisioner` (Job hook PreSync ArgoCD dans chaque overlay) crée chaque topic de flux plus son homologue `.dlq` en un seul appel admin idempotent. MSK tourne avec `auto.create.topics.enable=false` (propriété serveur explicite), donc un topic existe **parce qu'il** figure dans le registre — un nom de topic mal orthographié fait échouer la synchronisation au lieu d'engendrer un topic fantôme avec des défauts que personne n'a choisis.

### 2.5 Frontières de contrôle sécurité et conformité TIER-0

- **Immuabilité de l'audit** imposée sur quatre domaines de confiance indépendants : (1) registre haché en chaîne, INSERT-only au niveau applicatif (Postgres) ; (2) **S3 Object-Lock en mode COMPLIANCE** (écriture unique, non supprimable même par le compte racine avant rétention) ; (3) SSE-KMS sous une **KEK** dédiée ; (4) points de contrôle Merkle signés ancrés dans le bucket WORM comme témoin externe.
- **Garde au moindre privilège (IRSA) :** le rôle `audit` est le *principal unique* habilité à `kms:Decrypt/GenerateDataKey` sur la KEK et en **écriture seule (pas de `DeleteObject`)** sur le bucket WORM ; le rôle `media` est restreint à la lecture/écriture d'objets sur son seul bucket. Les rôles ne sont créés que si les ARN de ressource existent (dev non affecté).
- **RGPD Art. 17 :** crypto-effacement — détruire la DEK par sujet ; la chaîne (sur le chiffré) reste vérifiable, la preuve survit, la conservation légale prévaut.
- **Posture fail-closed :** les services TIER-0 refusent plutôt que de se dégrader (p. ex. break-glass refusé si l'écriture n'est pas enregistrée).
- **Isolation de la périphérie :** la seule entrée publique est le plan WSS temps réel via un **NLB L4** (TLS terminé en périphérie, la poignée de main WS brute atteint le pod) — délibérément *pas* un ALB L7, afin que la passerelle, et non un proxy, détienne la table de connexions.

### 2.6 Livraison GitOps

App-of-AppSets ArgoCD, amorcé par environnement (`bootstrap/` et `bootstrap/staging/`) : **opérateurs** (CNPG, scylla-operator, External Secrets, **KEDA**, k6) en sync-wave **−10**, puis **sécurité** (cert-manager, AWS LB controller, external-dns), **plateforme** (Karpenter, metrics-server), **observabilité** (monitoring), et **charges de travail** en wave **0**. Le séquencement est structurant : les CRD des opérateurs (`ScaledObject`, `Cluster`, `ExternalSecret`) doivent exister avant que l'overlay applique les CR qui les référencent.

---

## 3. Runbook de Déploiement et d'Amorçage (staging → production)

### 3.1 Ordre des dépendances de provisionnement

Terragrunt résout le DAG via `run-all apply` ; l'ordre explicite (chacun consommant les sorties du précédent) est :

```
1. networking/vpc                      # VPC, sous-réseaux, CIDR
2. eks                                  # cluster, provider OIDC, node groups (system + database)
3. data/msk                            # brokers Kafka + secret SCRAM
   data/elasticache                    # endpoint Redis + secret AUTH
   data/opensearch                     # domaine + secret maître
   data/media-bucket                   # bucket S3 media
   data/audit-kms                      # KEK d'audit
   data/audit-worm                     # bucket Object-Lock (dépend de audit-kms)
4. security/irsa-roles                 # ESO + rôles applicatifs audit/media — CONSOMME les ARN data, donc APRÈS l'étape 3
5. kubernetes/argocd                   # ArgoCD ; écrit global-params-staging.json
6. (GitOps) convergence des opérateurs (wave -10) : CNPG, scylla-operator, ESO, KEDA
7. kubectl apply -k k8s/base/infra/scylla-cluster   # ScyllaCluster dans le ns `scylla` (FQDN non préfixé)
8. (GitOps) synchronisation des charges de travail (wave 0) : k8s/overlays/staging
```

**Note d'ordonnancement critique :** `security/irsa-roles` dépend désormais des sorties de `audit-kms`/`audit-worm`/`media-bucket` — il doit s'exécuter **après** les magasins de données (réorganisation par rapport à l'agencement historique ; les `mock_outputs` autorisent un `plan` à blanc en amont).

### 3.2 Gestion des secrets et des identifiants

- **Générés par la machine (Terraform → Secrets Manager → ESO) :** SCRAM MSK, AUTH Redis, maître OpenSearch. Synchronisés dans `backend-creds` (`envFrom` à l'échelle de la flotte) et `search-creds`.
- **`DATABASE_URL` :** issu du secret `<name>-app` généré par chaque cluster CNPG (clé `uri`), injecté via des patches par service à la fois dans l'**init-container du migrator** **et** dans le conteneur d'exécution (le migrator l'exige impérativement).
- **À provisionner hors-bande (à créer manuellement dans Secrets Manager sous `core-platform-staging-*`) :**
  - `…-media-s3` `{access_key, secret_key}` — clés statiques d'un utilisateur IAM.
  - `…-audit-crypto` `{object/witness access+secret, kek_base64, signing_key_base64}`.
  - `…-auth-secrets` `{signing_private_pem, signing_public_pem, keycloak_client_secret}`.

### 3.3 Étapes manuelles et substituts (placeholders)

Les substituts d'endpoint (`<<…>>`) sont remplacés à partir des sorties Terragrunt au moment du déploiement (dans les fichiers `.env` et les patches de scaler KEDA) : `<<MSK_BOOTSTRAP_BROKERS_SASL_SCRAM>>`, `<<ELASTICACHE_CONFIG_ENDPOINT>>`, `<<OPENSEARCH_ENDPOINT>>`, `<<ACM_CERTIFICATE_ARN>>` (TLS du NLB), `<<KEYCLOAK_TOKEN_ENDPOINT>>`, `<<AUTH_JWKS_URL>>`. De plus : provisionner les secrets du §3.2 ; vérifier que chaque topic mis à l'échelle sur retard dispose de **≥ maxReplicaCount partitions** (`counter`=12, `realtime`=8, `audit`=4) ; construire/pousser les images via la CI matricielle `fleet-images-deploy` vers `:staging`.

### 3.4 Mises en garde Jour-1 et reports connus

1. **Modèle d'identifiants pour le magasin d'objets (action requise) :** `media` et `audit` construisent leurs clients S3/KMS avec des **identifiants statiques `rusty-s3`**, et *non* la chaîne d'identifiants du SDK AWS — donc les rôles IRSA, bien que provisionnés, ne sont **pas consommés par le code tel quel**. Le staging requiert des clés statiques d'utilisateur IAM injectées dans `media-s3-creds`/`audit-crypto`. Les rôles IRSA restent la cible correcte si le code migre vers le SDK.
2. **Reports externes d'audit :** le vrai AWS KMS et un véritable témoin WORM inter-comptes sont reportés (travail IAM/organisation) ; le câblage de staging utilise le **chemin KEK-ENV v1** avec le témoin pointé sur le même bucket WORM.
3. **Keycloak non provisionné :** l'IdP fédéré d'`auth` est externe et pas encore mis en place ; ses identifiants de courtage sont des substituts. Le plan WSS de `realtime` est fail-closed (`RTM-1001`) jusqu'à ce que le JWKS d'auth soit joignable — son plan health gRPC n'est pas affecté, le pod devient donc tout de même Ready.
4. **Étiquette `:staging` mutable :** ArgoCD ne redéploiera pas automatiquement sur un re-push d'étiquette sans Argo Image Updater ou un changement de digest ; l'étiquette `:<git-sha>` est disponible pour l'épinglage.
5. **Écart de charge utile en streaming :** le décalage de forme `post → geo-discovery/notification` (post n'émet ni lat/lng ni légende) reste une décision produit ouverte et suivie — le câblage est correct, la forme ne l'est pas.

---

## Annexe A — Allocation des ports

`chat` 50051 · `profile` 50052 · `social-graph` 50053 · `geo-discovery` 50054 · `notification` 50055 · `post` 50056 · `comment` 50057 · `engagement` 50058 · `account` 50059 · `auth` 50060 · `timeline` 50070 · `moderation` 50061 · `search` 50062 · `media` 50063 · `counter-server` 50064 · `counter-worker` 50065 · `realtime-gateway` 50066 (gRPC) + 8443 (WSS) · `realtime-dispatcher` 50067 · `audit-server` 50068 · `audit-worker` 50069.

## Annexe B — Catalogue des topics

**Producteurs :** `account.v1.events`, `profile.v1.events`, `post.{published,updated,deleted,v1.events}`, `comment.{created,deleted}`, `engagement.reactions`, `social-graph.{followed,unfollowed,blocked,author_tier_changed}`, `chat.*`, `counter.v1.popularity`, `moderation.v1.events`, `auth.v1.events`, `media.v1.events`. **Consommateurs différés :** `audit.v1.events`, `moderation.{reports,signals}`, `view/impression/click.v1.events`, `social-graph.follows`. **Producteurs orphelins (marge) :** `post.updated`, `social-graph.blocked`, `chat.{conversation.created,conversation.published,member.joined,member.left,message.sent}`.
