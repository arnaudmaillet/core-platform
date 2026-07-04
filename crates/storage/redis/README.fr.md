---
i18n:
  source: ./README.md
  source_sha256: fb3102a72bcc96f3d45bdec409882b1992672d21a1e9d942cb23dc57e769c679
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `redis-storage` — Client Redis instrumenté et agnostique de la topologie, sur le driver `fred`

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `storage` — capacité de transport/connexion Redis (sans clés, TTL, ni scripts) |
> | **Package** | `redis-storage` (dir : `crates/storage/redis`) |
> | **Consommé par** | `chat`, `profile`, `social-graph`, `engagement`, `geo-discovery`, `notification`, `timeline` |
> | **Dépend de** | `fred` 10.x, `telemetry`, `error`, `health` |
> | **Stabilité** | contrat stable |
> | **Feature flags** | hérite des features fred (p. ex. `subscriber-client` pour `SSUBSCRIBE`/`SPUBLISH`) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`redis-storage` est le crate d'infrastructure Redis partagé : une abstraction de client de qualité
production et pleinement instrumentée sur le driver [`fred`](https://crates.io/crates/fred) (v10.x),
câblant multiplexage automatique, agnosticisme de topologie, reconnexion à backoff exponentiel,
télémétrie OTel-native et un type d'erreur compatible `AppError` en une primitive réutilisable. Les
consommateurs importent `RedisClient` / `RedisPool` et utilisent les command traits de fred directement
(via `Deref`).

**Frontière architecturale** — il ne contient **aucune** clé de cache applicative, TTL, modèle de
domaine, script Lua, ni logique de rate-limit. Il n'expose que la capacité de transport et de connexion.

---

## 📐 Architecture & décisions clés

```
Consumers (CQRS, cache utils, rate-limit mw) ── RedisClient / RedisPool
        ▼
redis-storage: config(topology) · error(map) · listener(event) · client/pool(builder) · health(check)
        ▼  fred::types::Builder
fred 10.x (multiplexer, pipeline engine, reconnect policy)  ──TCP/TLS──► Cluster / Sentinel / Standalone
```

- **Un multiplexeur par client** — fred route *toutes* les commandes de *tous* les appelants à travers un
  unique writer d'arrière-plan lock-free, donc pas de verrou par commande ; les commandes du même tick
  s'auto-pipelinent en un flush (`REDIS_AUTO_PIPELINE`). Un `RedisPool` de taille N est N multiplexeurs
  indépendants pour les workloads liés à la bande passante d'écriture.
- **Topologie derrière une fonction** — `TopologyKind::into_server_config()` est l'unique traduction de la
  config env en `ServerConfig` de fred (`standalone`→Centralized, `cluster`→Clustered,
  `sentinel`→Sentinel). Ajouter une topologie ne touche que cet enum + cette fonction.
- **Erreurs mappées vers des `RDS-xxxx` stables** — chaque `fred::error::RedisError` se réduit en une
  variante `RedisStorageError` nommée avec code, `Severity`, retryabilité et statut HTTP, donc les
  consommateurs branchent sur le contrat plateforme, pas les internes de fred.
- **Télémétrie pontée à la construction** — les builders lancent un event listener pontant le cycle de vie
  connect/reconnect/error de fred vers le subscriber OTel global du processus, donc il **doit** être
  installé *avant* `build()`.

---

## 🔌 API publique & contrat

```rust
pub struct RedisClient { pub inner: fred::clients::RedisClient }   // single multiplexed connection
pub struct RedisPool   { pub inner: fred::clients::Pool }          // N connections (throughput-critical)
impl Deref for RedisClient/RedisPool { /* → fred client; use command traits directly */ }

pub struct RedisClientBuilder; impl { pub fn new(RedisConfig) -> Self; pub async fn build(self) -> Result<RedisClient, RedisStorageError>; }
pub struct RedisPoolBuilder;   impl { pub fn new(RedisConfig) -> Self; pub async fn build(self) -> Result<RedisPool, RedisStorageError>; }

pub async fn health_check<C: ClientLike + HeartbeatInterface>(client: &C) -> Result<(), RedisStorageError>;
pub fn spawn_event_listener<C: EventInterface>(client: &C) -> [JoinHandle<()>; 3];   // called by builders
```

> **Contrat :** les clients/pools `Deref` vers le client fred — appeler directement les command traits de
> fred (`KeysInterface`, `HashesInterface`, …). `RedisClient` est cloneable à bas coût. Le subscriber OTel
> doit être installé avant `build()` (fred émet des spans à la construction).

---

## 🧯 Modèle d'erreur

`RedisStorageError` (`#[non_exhaustive]`) implémente `error::AppError` ; la catégorie est toujours `"RDS"` :

| Code | Variant | Retryable | Severity | HTTP |
|---|---|---|---|---|
| RDS-1001 | `Timeout` | yes | High | 503 |
| RDS-1002 | `Disconnected` | yes | High | 503 |
| RDS-1003 | `Io` | yes | High | 503 |
| RDS-1004 | `Backpressure` | yes | High | 503 |
| RDS-1005 | `Canceled` | yes | Medium | 503 |
| RDS-2001 | `PoolExhausted` | yes | High | 503 |
| RDS-3001 | `Authentication` | no | Critical | 500 |
| RDS-4001 | `WrongType` | no | Low | 422 |
| RDS-4002 | `InvalidArgument` | no | Low | 422 |
| RDS-4003 | `InvalidCommand` | no | Medium | 500 |
| RDS-4004 | `NotFound` | no | Low | 404 |
| RDS-5001 | `Cluster` | yes | High | 503 |
| RDS-7001 | `Sentinel` | yes | High | 503 |
| RDS-8001..8004 | `Configuration`/`Tls`/`Protocol`/`Parse` | no | Crit/Crit/Crit/Medium | 500 |
| RDS-9000 | `Unknown` | no | Medium | 500 |

---

## 📦 Intégration

```toml
[dependencies]
redis-storage = { workspace = true }
```

```rust
use fred::interfaces::{KeysInterface, HashesInterface};
use redis_storage::{RedisConfig, RedisPoolBuilder, health::health_check};

telemetry::init(telemetry::Config::from_env()).await?;          // BEFORE build — fred emits into the subscriber
let pool = RedisPoolBuilder::new(RedisConfig::from_env()).build().await?;
health_check(&pool).await?;
pool.set::<(), _, _>("session:42", "payload", None, None, false).await?;  // fred traits via Deref
```

---

## ⚙️ Configuration & feature flags

**Connexion / topologie :** `REDIS_TOPOLOGY` (`standalone`|`cluster`|`sentinel`, défaut `standalone`),
`REDIS_HOSTS` (défaut `127.0.0.1:6379`), `REDIS_USERNAME`/`REDIS_PASSWORD`, `REDIS_DATABASE` (0–15,
ignoré en cluster). **Sentinel :** `REDIS_SENTINEL_SERVICE_NAME` (défaut `mymaster`) +
`REDIS_SENTINEL_USERNAME`/`PASSWORD`. **Tuning :** `REDIS_CONNECTION_TIMEOUT_SECS` (5),
`REDIS_COMMAND_TIMEOUT_MS` (3000 ; 0 désactive), `REDIS_FAIL_FAST` (true),
`REDIS_UNRESPONSIVE_TIMEOUT_SECS` (60). **Pool :** `REDIS_POOL_SIZE` (8). **Pipelining :**
`REDIS_AUTO_PIPELINE` (true), `REDIS_PIPELINE_BATCH_SIZE` (200), `REDIS_MAX_COMMAND_BUFFER_LEN`
(10000). **Reconnect :** `REDIS_RECONNECT_MIN/MAX_DELAY_MS` (100 / 30000), `REDIS_RECONNECT_MAX_ATTEMPTS`
(0 = illimité), `REDIS_RECONNECT_MULTIPLIER` (2). **Cluster :** `REDIS_MAX_REDIRECTIONS` (défaut fred 16).

**Feature flags :** hérite de celles de fred — notamment `subscriber-client` (activé transitivement pour
les services utilisant le pub/sub shardé `SSUBSCRIBE`/`SPUBLISH`, p. ex. `chat`).

---

## 🔭 Observabilité

fred émet des spans `fred.command` (`DEBUG`, `db.system=redis`, `net.peer.*`). L'event listener émet des
événements connect/reconnect (`INFO`) et erreur de connexion (`ERROR`, `error.message`), tous
`otel.kind=CLIENT`.

Alertes suggérées : taux `RDS-1001` > 10/5m ⇒ high (réseau) ; tout `RDS-3001` ⇒ critique (creds) ;
`RDS-2001` soutenu ⇒ high (pool sous-dimensionné) ; pics `RDS-5001` ⇒ high (failover cluster).

---

## 🧪 Tests

```bash
cargo test   -p redis-storage                 # unit — no Redis
cargo clippy -p redis-storage --all-targets
# integration (live):
REDIS_HOSTS=127.0.0.1:6379 cargo test -p redis-storage -- --include-ignored
REDIS_TOPOLOGY=cluster REDIS_HOSTS=127.0.0.1:7000,7001,7002 cargo test -p redis-storage -- --include-ignored
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `RedisStorageError::Configuration` au démarrage — liste d'hôtes vide.**
`REDIS_HOSTS` est vide/espaces. `from_env()` exige au moins un `host:port` valide. Définir
`REDIS_HOSTS=127.0.0.1:6379`.

**2. `Disconnected` avec `fail_fast = true` — le service ne démarre pas.**
Redis n'était pas joignable au boot (course avec la santé du conteneur, ou il est down). Ajouter une sonde
de readiness Redis + `depends_on`, ou mettre `REDIS_FAIL_FAST=false` pour entrer dans la boucle de
reconnexion au lieu d'échouer, ou relever `REDIS_CONNECTION_TIMEOUT_SECS` sur les réseaux à RTT élevé.

**3. `RDS-2001 PoolExhausted` soutenu sous charge.**
`REDIS_POOL_SIZE` trop petit (chaque membre est une connexion TCP). Le relever (16→32, en profilant
d'abord le nombre de connexions côté serveur), relever `REDIS_MAX_COMMAND_BUFFER_LEN` pour absorber les
pics, confirmer `REDIS_AUTO_PIPELINE=true` (amortit le RTT). Si le CPU de Redis est le goulet, scaler
Redis horizontalement.

**4. Pas de spans pour mes commandes, ou événements de cycle de vie manquants.**
Le subscriber OTel n'a pas été installé avant `build()`. Appeler `telemetry::init(...)` d'abord — fred
câble ses hooks de tracing dans le subscriber *actif* au moment de la construction.
