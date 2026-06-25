---
i18n:
  source: ./README.md
  source_sha256: 1400e3bb1129f8d2e49c4eb74e176696bcca557fff116b98dc7b783fb9dab25b
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `postgres` — Stockage PostgreSQL topology-aware avec routage de shard déterministe

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `storage` — pool de connexions + routeur de shard + transaction manager (sans schéma) |
> | **Package** | `postgres` (dir : `crates/storage/postgres`) |
> | **Consommé par** | `account` (et tout futur service adossé à Postgres) |
> | **Dépend de** | `sqlx`, `seahash`, `tokio`, `telemetry`, `error`, `health` |
> | **Stabilité** | contrat stable (API `run_on_shard` figée) |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`postgres` est la couche d'infrastructure PostgreSQL de référence : un pool de connexions de qualité
production, un routeur de shard déterministe et un transaction manager topology-aware derrière une seule
API ergonomique. Le code écrit contre `run_on_shard()` compile et se comporte identiquement sous les deux
topologies `SingleNode` (CockroachDB / Aurora) et `ApplicationSharded` (sharding manuel) — **zéro
changement au site d'appel**.

**Frontière architecturale** — il possède le cycle de vie des connexions, le routage et les transactions ;
il ne possède **aucun schéma, aucune migration, aucun modèle de domaine**. L'atomicité inter-shards est
volontairement hors périmètre : `run_on_shard` est ACID *au sein d'un seul shard* ; la cohérence
inter-shards doit utiliser le pattern outbox ou des sagas.

---

## 📐 Architecture & décisions clés

```
SingleNode (PG_TOPOLOGY=single)            ApplicationSharded (PG_TOPOLOGY=sharded)
TransactionManager(Arc<Topology>)          TransactionManager(Arc<Topology>)
   run() / run_on_shard()                     run_on_shard(key, f)
        ▼                                          ▼ deterministic_shard_id(key, n) = SeaHash % shard_count
   PgPool (global, engine-routed)             ShardCluster → PgPool[0..N]
   CockroachDB / Aurora                       shard-0 … shard-N
```

- **`ShardKey` basé sur le feed, pas `&[u8]`** — une signature `fn shard_bytes(&self) -> &[u8]` ne peut
  renvoyer une référence vers des octets sur la pile (`u64::to_le_bytes()` est droppé en fin de méthode).
  Imiter `std::hash::Hash` (les implémenteurs poussent des octets dans un `Hasher` générique) est sans
  allocation pour tout type.
- **Déterminisme + immutabilité** — `(key bytes, shard_count)` mappe toujours vers le même `ShardId` à
  travers redémarrages/machines ; `ShardCluster` est construit une fois derrière `Arc`, sans verrou sur le
  chemin de lecture.
- **Build de cluster fail-fast** — `PgClusterBuilder` refuse de démarrer si un pool de shard échoue à se
  connecter ; un cluster partiel n'est jamais autorisé à l'exécution.
- **La politique de retry vit en amont** — `Deadlock`/`SerializationFailure`/`PoolTimedOut` sont marqués
  retryable mais **pas** réessayés ici ; c'est le travail de la couche `resilience` / du middleware CQRS.

---

## 🔌 API publique & contrat

```rust
pub struct TransactionManager { /* Arc<Topology> */ }
pub type PgTransaction = sqlx::Transaction<'static, sqlx::Postgres>;
pub enum TopologyConfig { SingleNode(PostgresConfig), ApplicationSharded(ShardedPostgresConfig) }

pub trait ShardKey { fn hash_shard_key<H: std::hash::Hasher>(&self, state: &mut H); }  // mirrors std::hash::Hash

impl TransactionManager {
    pub fn new(pool: PgPool) -> Self;                  // SingleNode
    pub fn from_cluster(cluster: ShardCluster) -> Self;// ApplicationSharded
    pub fn pool(&self) -> &PgPool;                     // panics in sharded mode
    pub fn pool_for<K: ShardKey + ?Sized>(&self, key: &K) -> Result<&PgPool, StorageError>;

    /// SingleNode only — ShardRoutingFailed in sharded mode. Backward-compatible with old call sites.
    pub async fn run<F, T, E>(&self, f: F) -> Result<T, E> where /* F: FnOnce(&mut PgTransaction) -> BoxFuture<…> */;
    /// Topology-agnostic — PREFERRED. Key ignored in SingleNode; routes deterministically when sharded.
    pub async fn run_on_shard<K: ShardKey + ?Sized, F, T, E>(&self, key: &K, f: F) -> Result<T, E>;
}
```

`ShardKey` a des impls blanket pour `Uuid`, `String`/`str`, `u64`/`u128`/`i64`, `[u8; N]`/`[u8]` (toutes
sans allocation ; les types DST nécessitent le bound `?Sized`).

> **Contrat :** le code de service nouveau doit utiliser `run_on_shard` exclusivement — il est agnostique
> de la topologie. `TransactionManager::clone()` est O(1) (un bump d'`Arc`). Le `Drop` de `Transaction`
> sqlx émet un rollback best-effort si l'exécuteur est annulé en cours de transaction.

---

## 🧯 Modèle d'erreur

`StorageError` (15 variantes) implémente `error::AppError` :

| Code | Variant | Severity | Retryable |
|---|---|---|---|
| DB-1001 | `UniqueViolation` | Low | No |
| DB-1002 | `ForeignKeyViolation` | Medium | No |
| DB-1003 | `NotNullViolation` | Medium | No |
| DB-1004 | `CheckViolation` | Low | No |
| DB-2001 | `Deadlock` | High | **Yes** |
| DB-2002 | `SerializationFailure` | High | **Yes** |
| DB-3001 | `PoolTimedOut` | High | **Yes** |
| DB-3002 | `PoolClosed` | Critical | No |
| DB-4001 | `RowNotFound` | Low | No |
| DB-5001 | `Migration` | Critical | No |
| DB-6001 | `Connection` | High | No |
| DB-7001 | `Configuration` | Critical | No |
| DB-8001 | `ShardNotFound` | Critical | No |
| DB-8002 | `ShardRoutingFailed` | Critical | No |
| DB-9000 | `Database` | Medium | No |

---

## 📦 Intégration

```toml
[dependencies]
postgres = { workspace = true }
```

```rust
use postgres::{TopologyBuilder, TopologyConfig, TransactionManager};

let tx_manager: TransactionManager = TopologyBuilder::build(TopologyConfig::from_env()).await?;

// Topology-agnostic — unchanged whether SingleNode or ApplicationSharded.
tx_manager.run_on_shard(&account_id, |tx| Box::pin(async move {
    sqlx::query("INSERT INTO accounts (id) VALUES ($1)").bind(account_id)
        .execute(&mut **tx).await.map_err(MyError::from)
})).await?;
```

`health_check(pool)` / `health_check_cluster(cluster)` (sonde tous les shards en concurrence) adossent le
readiness du service.

---

## ⚙️ Configuration & feature flags

**SingleNode** (`PG_TOPOLOGY=single` ou non défini) :

| Variable | Required | Default | Description |
|---|---|---|---|
| `DATABASE_URL` | **Yes** | — | libpq connection string |
| `PG_MAX_CONNECTIONS` / `PG_MIN_CONNECTIONS` | No | `20` / `2` | Pool ceiling / warm idle |
| `PG_ACQUIRE_TIMEOUT_SECS` | No | `5` | Max wait for a free slot → `PoolTimedOut` |
| `PG_IDLE_TIMEOUT_SECS` / `PG_MAX_LIFETIME_SECS` | No | `600` / `1800` | Idle reaping / max age |
| `PG_SLOW_STATEMENT_THRESHOLD_MS` | No | `1000` | WARN threshold for slow queries |

**ApplicationSharded** (`PG_TOPOLOGY=sharded`) : tout ce qui précède par shard, plus `PG_SHARD_COUNT`
(**immuable après le premier déploiement** — le changer remappe chaque clé) et `PG_SHARD_<N>_URL` pour
chaque `N` dans `[0, PG_SHARD_COUNT)`.

Aucune feature cargo.

---

## 🔭 Observabilité

Spans OTel : `postgres.pool.build`, `postgres.cluster.build` (`shard_count`), `postgres.topology.build`,
`db.transaction`, `db.transaction.sharded` (`shard_id`), `postgres.health_check[_cluster]`.

Alertes suggérées : taux `DB-3001` > 0 pendant 30s ⇒ page ; `DB-2001` > 5/min ⇒ warn ; tout `DB-8001` ⇒
page (misconfig) ; p99 latence requête > seuil lent ⇒ warn.

---

## 🧪 Tests

```bash
cargo test   -p postgres --lib            # routing/hashing/error-mapping/ShardKey — no DB
cargo clippy -p postgres --all-targets
docker compose up -d postgres
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo test -p postgres
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `DB-8002 ShardRoutingFailed` à l'exécution.**
`run()` (sans clé) a été appelé alors que `PG_TOPOLOGY=sharded` — il ne peut choisir un shard. Remplacer
par `run_on_shard(&shard_key, …)` ; toute valeur identifiant uniquement l'entité propriétaire
(`AccountId`) convient.

**2. `DB-8001 ShardNotFound` au démarrage / première requête.**
`PG_SHARD_COUNT` ne correspond pas à l'ensemble des vars `PG_SHARD_<N>_URL`, ou une URL manque. Chaque
entier dans `[0, PG_SHARD_COUNT)` a besoin d'une URL — corriger l'env et redémarrer.

**3. `DB-3001 PoolTimedOut` sous charge.**
`PG_MAX_CONNECTIONS` trop bas, ou une longue transaction tient des connexions. Relever le plafond ; en
mode shardé, inspecter les durées de span `db.transaction.sharded` par shard pour un shard chaud (skew de
clé) épuisant son pool pendant que d'autres sont inactifs.

**4. Changer `PG_SHARD_COUNT` a « rééquilibré » tout de travers.**
Il est **immuable** après le premier déploiement — le changer remappe chaque clé vers un shard différent.
Un changement de nombre de shards est une opération de re-sharding complète des données, pas un réglage de
config.
