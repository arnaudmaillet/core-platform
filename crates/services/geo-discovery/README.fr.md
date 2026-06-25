---
i18n:
  source: ./README.md
  source_sha256: 66ea5a03172ddeae928c31dd9bb443316b0d3304d6912bffd5fd36c478287979
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, topics Kafka, identifiants) sont volontairement laissés en anglais.

# `geo-discovery` — Index spatial H3 temps réel pour le fil carte mondial, servi en deux allers-retours Redis

> **Fiche service**
>
> | | |
> |---|---|
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |
> | **Astreinte / escalade** | `<TODO: rotation-astreinte>` → `<TODO: politique-escalade>` |
> | **Palier (Tier)** | **TIER-1** — surface de lecture seule ; dégradable vers ScyllaDB |
> | **Binaire déployable** | `crates/apps/geo-discovery-server` (crate bibliothèque : `crates/services/geo-discovery`) |
> | **Bases de données** | Redis (index ZSET + cache de cartes msgpack) · ScyllaDB keyspace `geo_discovery` |
> | **Asynchrone** | ne publie rien · consomme `post.published` / `engagement.score_updated` / `profile.tier_changed` |
> | **Appelants amont** | `<TODO: BFF / clients carte>` |
> | **Dépendances aval** | Redis, ScyllaDB, Kafka |
> | **SLO** | requête de tuile p99 **< 50 ms** à l'échelle continentale |

---

## 🎯 Vue d'ensemble & rôle du service

`geo-discovery` est le moteur d'ingestion + requête géospatiale derrière la carte interactive mondiale.
Chaque post publié est encodé en cellules hexagonales Uber H3 à trois résolutions, scoré par viralité
temps réel, et servi comme carte (« map card ») entièrement hydratée en **deux allers-retours Redis** —
sans fan-out vers `post` ou `profile` au moment de la requête.

Le problème difficile qu'il résout est qu'**un fil carte mondial avec 100 M de posts actifs ne peut pas
se permettre des lookups gRPC N+1 à chaque panoramique de viewport**. Il résout cela avec une projection
read-model indépendante : les champs de carte (`author_handle`, `thumbnail_url`, `author_avatar_url`,
`author_tier`) sont dénormalisés à l'ingestion, donc les données d'affichage sont entièrement locales.
L'état relationnel dynamique (ami/abonné) est résolu côté client, préservant un cache de cartes *partagé*
et évitant des variantes de cache en O(utilisateurs × posts).

**Objectifs fondamentaux :** requête de tuile sub-50 ms P99 (pipeline Redis + MGET) ; ~9 Go de Redis à
100 M de posts (cap Top-K + éviction à froid + filtrage en loi de puissance) ; zéro fan-out au moment de
la requête ; TTL par défaut 48 h (30 j pour premium), imposé via `USING TTL` Scylla et `EX` Redis.

---

## 📐 Architecture & concepts

La surface gRPC est **en lecture seule** — toutes les écritures arrivent via des workers Kafka.

```
WRITE: post.published          ─► PostIndexerWorker  (H3 encode R5/7/9 → Scylla INSERT ×4 → Redis ZADD+cap ×3 → card SET if score≥θ)
       engagement.score_updated ─► ScoreUpdaterWorker (Scylla UPDATE score → ZADD XX ×3, skip-if-absent)
       profile.tier_changed     ─► TierSyncWorker     (Scylla UPDATE author_tier → Redis DEL card)
       (60s tick)               ─► TilePrunerWorker    (PRUNE_COLD_TILES Lua → DEL cold tile ZSETs)

READ:  QueryTile ─► zoom→resolution ─► viewport→grid_disk (≤50 tiles)
                 ─► Phase 1: ZRANGEBYSCORE ×N (1 RTT via fred mux) → post_ids
                 ─► Phase 2: MGET cards ×M (1 RTT)
                 ─► Phase 3 (miss): Scylla get_card (Fast profile)
```

**Taxonomie Redis :** `sg:geo:tile:{h3}:{res}` (ZSET, score=viralité, élagué),
`sg:geo:card:{post_id}` (STRING, `MapPostCard` msgpack, `EX ttl`), `sg:geo:hot_tiles` (ZSET, époque de
dernier accès par tuile). **ScyllaDB :** `posts_by_tile` (TWCS, PK `(h3_index, resolution)` — composite
pour éviter les shards urbains chauds), `map_post_cards` (LCS, PK `post_id` — lectures par point pures,
une seule colonne de score mutable).

Trois scripts Lua atomiques pilotent le chemin chaud : `ZADD_TOPK` (cap par tuile, évince le plus bas au
débordement), `ZADD_XX` (mise à jour seulement si le membre est présent — les posts évincés ne sont jamais
réinsérés), `PRUNE_COLD_TILES` (évince les tuiles inactives au-delà du seuil de froid).

> ⚠️ **Note cluster :** `PRUNE_COLD_TILES` construit les clés de tuile dans Lua et **n'est pas sûr en
> Redis Cluster** (`DEL` cross-slot). Ce service suppose un Redis standalone / mono-shard.

> **Invariants :** le mapping zoom→résolution avec planchers de viralité (plancher R5 500 / R7 50 ou 5 /
> R9 0) et caps Top-K (200/500/1000) borne la RAM par tuile quelle que soit la densité urbaine.

---

## 📊 Objectifs de niveau de service (SLO)

| SLI | Objectif | Fenêtre | Mesuré par |
|---|---|---|---|
| `QueryTile` p99 | **< 50 ms** | 1 h | `geo_discovery_tile_query_duration_ms` |
| Taux de cache miss | < 0,30 | 5 min | `geo_discovery_cache_miss_ratio` |
| Lag d'ingestion `post.published` | < 30 s | direct | `geo_discovery_post_indexer_lag_seconds` |
| Lag `engagement.score_updated` | < 10 s | direct | `geo_discovery_score_updater_lag_seconds` |
| RAM spatiale Redis (tuiles chaudes) | < 50 000 tuiles | direct | `geo_discovery_hot_tile_count` |

**Budget d'erreur :** `<TODO>`. **En cas d'épuisement :** `<TODO>`. Les données de la carte sont
acceptablement périmées dans la fenêtre de rétention de 48 h, donc les SLO de lag d'ingestion sont plus
souples que le SLO de latence de requête.

---

## 🔗 Dépendances & rayon d'impact (blast radius)

**Aval :**

| Dependency | Purpose | If down → | Degradation |
|---|---|---|---|
| Redis | index ZSET + cache de cartes | la latence de requête monte | **Souple** — les lectures retombent sur Scylla (pas une panne) |
| ScyllaDB (`geo_discovery`) | source de vérité durable | l'ingestion réessaie ; les lectures froides échouent | **Dur** pour le chemin froid ; requêtes servies par Redis non affectées |
| Kafka | ingestion (post/score/tier) | les données de carte deviennent périmées | **Souple** — les requêtes servent encore les données en cache |

**Amont (rayon d'impact) :**

| Caller | Uses | Impact si `geo-discovery` est indisponible |
|---|---|---|
| `<TODO: BFF / clients carte>` | `QueryTile`, `GetCard` | le fil carte cesse de charger |

> **Chemin critique ?** Oui pour la surface carte spécifiquement ; c'est un read-model dérivé, donc une
> panne totale dégrade la carte mais rien d'autre.

---

## 🔌 Interfaces publiques & contrat d'API

### gRPC — `geo_discovery.v1.GeoDiscoveryService`

```protobuf
service GeoDiscoveryService {
  rpc QueryTile (QueryTileRequest) returns (QueryTileResponse);
  rpc GetCard   (GetCardRequest)   returns (GetCardResponse);
}
message QueryTileRequest { Viewport viewport = 1; int32 zoom_level = 2; }  // zoom ∈ [0,15]
message MapPostCard { string post_id=1; string author_id=2; string author_handle=3;
  string author_avatar_url=4; string thumbnail_url=5; int64 h3_index_r7=6;
  float virality_score=7; int64 published_at_ms=8; AuthorTier author_tier=9; }
```

> **Contrat de sérialisation :** `AuthorTier` est basé sur 0 **avec** un défaut sûr `UNSPECIFIED=0`
> (= Standard) ; `STANDARD=1, PREMIUM=2, VIP=3`. Rendu du badge : `author_tier` → badge statique ;
> `is_friend`/`is_following` sont délibérément **absents** (résolus côté client depuis le graphe social de
> session).

### Ports Rust (contrat hexagonal)

```rust
pub trait SpatialIndex: Send + Sync { /* upsert (ZADD+cap), update_score (ZADD XX), query (ZRANGEBYSCORE), touch_hot_tiles */ }
pub trait CardStore:    Send + Sync { /* set, mget (same-length Vec, None=miss), del */ }
pub trait TileRepository: Send + Sync { /* insert_tile_entry, upsert_card, update_card_score/tier, get_card, list_tile_post_ids */ }
```

### Contrat d'erreur (`GEO-xxxx`)

| Code | HTTP | Meaning |
|---|---|---|
| GEO-1001/1002 | 422 | coords outside WGS-84 / invalid H3 index |
| GEO-2001/2002 | 422 | viewport SW≥NE / zoom outside [0,15] |
| GEO-4001 | 500 | Lua returned unexpected value |
| GEO-5001/5002 | 500 | msgpack ser / deser failure |
| GEO-9001..9003 | 422 | malformed UUIDs / domain violation |

---

## 📨 Contrat événementiel & asynchrone

**Publie :** rien — `geo-discovery` est un matérialiseur read-model pur.

**Consomme :**

| Topic | Consumer group | Purpose | On poison/exhaustion |
|---|---|---|---|
| `post.published` | `geo-discovery-post-indexer` | H3 index + card projection | DLQ `{topic}.dlq` |
| `engagement.score_updated` | `geo-discovery-score-updater` | virality score sync (ZADD XX) | DLQ `{topic}.dlq` |
| `profile.tier_changed` | `geo-discovery-tier-sync` | author tier sync + card invalidation (one event per `post_id`, stateless) | DLQ `{topic}.dlq` |

> **Contrat d'exécution (obligatoire) :** les trois workers s'exécutent sous `run_consumer` — réessai sur
> place avec backoff + jitter (≤5 tentatives), dead-letter à l'épuisement et commit au-delà pour qu'une
> partition ne stagne jamais. At-least-once ; toutes les écritures idempotentes. Scylla est la source de
> vérité durable ; les ZSETs Redis se repeuplent au rejeu depuis `earliest`.

---

## 🌩️ Modes de défaillance & dégradation

| Failure | Symptom | Service behavior | Operator action |
|---|---|---|---|
| Redis indisponible | la latence de requête monte | **Souple** — l'écriture Scylla réussit ; la requête se dégrade en lectures Scylla complètes | se rétablit au reconnect |
| ScyllaDB indisponible | l'ingestion réessaie | `run_consumer` réessaie→DLQ ; les lectures froides échouent | drainer la DLQ une fois Scylla rétablie |
| Lag de consommateur | données de carte périmées | chemin de requête non affecté (lectures Redis/Scylla) | scaler les réplicas de consommateur |
| Pression mémoire Redis | `hot_tile_count` grimpe | TilePruner évince toutes les 60 s ; le cap Top-K borne par tuile | baisser `GEO_TILE_COLD_THRESHOLD_SECS` |
| Tempête d'événements de score | — | `ZADD_XX` ignore les membres absents ; pas d'inflation de ZSET | auto-limitant |

**Backpressure & limites.** Cap Top-K à chaque `ZADD` ; éviction des tuiles froides toutes les 60 s ;
viewport plafonné à ≤50 tuiles H3 par requête. Les écritures utilisent Strict (`LocalQuorum`), les
lectures Fast (`LocalOne` + spéculatif).

---

## 📦 Intégration & utilisation

```toml
[dependencies]
geo-discovery = { path = "crates/services/geo-discovery" }
```

Bibliothèque uniquement. Implémente [`service_runtime::Service`](../../platform/service-runtime/README.md)
sous le nom `geo_discovery::service::GeoDiscoveryService` — `build` construit les clients Scylla/Redis,
instancie `RedisGeoSpatialIndex`/`RedisCardStore`/`ScyllaTileRepository`, enregistre `QueryTileHandler`
(surface en lecture seule ; les écritures arrivent via Kafka), et lance les trois workers +
`TilePrunerWorker` ; `register` ajoute les services gRPC + réflexion ; `health_probes` vérifie
Scylla/Redis.

### Bootstrap (`crates/apps/geo-discovery-server`)

```rust
use std::net::SocketAddr;
use geo_discovery::service::GeoDiscoveryService;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let addr: SocketAddr = std::env::var("GEO_GRPC_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:50054".to_owned())
        .parse()?;
    service_runtime::serve::<GeoDiscoveryService>(addr).await
}
```

---

## ⚙️ Configuration & environnement d'exécution

| Variable | Required | Default | Description |
|---|---|---|---|
| `SCYLLA_URI` | **Yes** | — | ScyllaDB contact points. |
| `REDIS_URL` | **Yes** | — | Redis connection URL (standalone / single-shard). |
| `KAFKA_BROKERS` | **Yes** | — | Kafka brokers. |
| `GEO_GRPC_ADDR` | No | `0.0.0.0:50054` | gRPC bind address. |
| `GEO_CARD_CACHE_THRESHOLD` | No | `10.0` | Min virality to cache a card (power-law filter). |
| `GEO_DEFAULT_RETENTION_SECS` | No | `172800` | Default post TTL (48 h). **Must match Scylla `default_time_to_live`.** |
| `GEO_TILE_PRUNER_INTERVAL_SECS` | No | `60` | Cold-tile eviction tick. |
| `GEO_TILE_COLD_THRESHOLD_SECS` | No | `1800` | Inactivity window before a tile ZSET is evicted. |
| `GEO_POST_INDEXER_GROUP_ID` / `GEO_SCORE_UPDATER_GROUP_ID` / `GEO_TIER_SYNC_GROUP_ID` | No | service-specific | Kafka consumer groups. |

> Aucun flag de feature de compilation. `build.rs` compile `proto/geo_discovery/v1/*.proto`. Profils
> ScyllaDB : Strict (`LocalQuorum`) pour les mutations, Fast (`LocalOne` + spéculatif) pour les lectures.

---

## 🚀 Déploiement, migrations & rollback

- **Migrations :** `0001_create_keyspace.cql` → `0002_create_posts_by_tile_table.cql` →
  `0003_create_map_post_cards_table.cql` sur `geo_discovery`, appliquées **avant** le premier démarrage.
- **Pièges liés à l'état :** `GEO_DEFAULT_RETENTION_SECS` doit égaler le TTL de la table Scylla ; la clé de
  partition composite `(h3_index, resolution)` et le mapping zoom→résolution sont des contrats de lecture.
- **Cold-start :** les workers rejouent depuis `earliest` ; les ZSETs Redis se repeuplent automatiquement.
  Sûr à déployer.

---

## 📈 Télémétrie, performance & métriques

- **Runtime :** Tokio multi-thread (requis — écritures Scylla+Redis concurrentes via `tokio::join!`).
  `h3o` est en Rust pur. Plancher mémoire ~512 Mo ; `ulimit -n ≥ 4096`.

| Signal | Why it matters | Suggested alert |
|---|---|---|
| `geo_discovery_tile_query_duration_ms` | query SLO | p99 > 50 ms ⇒ page |
| `geo_discovery_cache_miss_ratio` | Redis offload health | > 0.30 for 5m ⇒ investigate |
| `geo_discovery_hot_tile_count` | spatial RAM | > 50 000 ⇒ tune pruner |
| `geo_discovery_post_indexer_lag_seconds` | ingest freshness | > 30 s ⇒ scale consumers |
| `geo_discovery_card_serialization_errors` | schema/memory | > 0 in 1m ⇒ investigate |

**Budget mémoire Redis (référence) :** ~0,7 Go à 10 M de posts, ~7 Go à 100 M. Leviers : `top_k_cap` par
résolution, `GEO_CARD_CACHE_THRESHOLD`, `GEO_TILE_COLD_THRESHOLD_SECS`.

---

## 🛠️ Développement local

```bash
docker compose up -d scylla redis kafka       # repo-root compose
for f in crates/services/geo-discovery/migrations/*.cql; do cqlsh 127.0.0.1 9042 -f "$f"; done
cargo build -p geo-discovery && cargo clippy -p geo-discovery -- -D warnings
cargo test  -p geo-discovery
SCYLLA_URI=127.0.0.1:9042 REDIS_URL=redis://127.0.0.1:6379 KAFKA_BROKERS=127.0.0.1:9092 cargo run -p geo-discovery
```

---

## 🚨 Dépannage & runbook

> Format : **symptôme → cause racine → mitigation.**

**1. `QueryTile` renvoie `tile_count > 0` mais des `cards` vides.**
Cause racine (la plus fréquente) : ZSETs vides mais Scylla a des lignes ⇒ le `geo-discovery-post-indexer`
est en retard ; ou ZSETs peuplés mais cartes vides ⇒ `GEO_CARD_CACHE_THRESHOLD` trop élevé. Mitigation :
vérifier `kafka-consumer-groups --describe --group geo-discovery-post-indexer` et scaler ; ou abaisser le
seuil à `1.0` temporairement pour confirmer l'apparition des cartes. Vérifier que le client n'envoie pas
des coordonnées SW/NE inversées.

**2. La mémoire Redis croît sans borne.**
Cause racine : TilePruner a crashé (chercher `tile pruner worker started` ;
`geo_discovery_tile_pruner_evictions` bloqué à 0) ou `GEO_TILE_COLD_THRESHOLD_SECS` trop élevé.
Mitigation : redémarrer/redéployer ; abaisser le seuil de froid à `900`. Urgence : `redis-cli FLUSHDB`
(annoncer une maintenance) — les prochaines requêtes cold-start depuis Scylla.

**3. Les mises à jour de score ne se reflètent pas sur la carte.**
Cause racine : le post a été évincé Top-K (`ZADD_XX` ignore les membres absents — attendu ; se rafraîchit
au TTL), ou `geo-discovery-score-updater` a du lag de consommateur. Mitigation : comparer le
`map_post_cards.virality_score` Scylla au `ZSCORE` Redis ; si Scylla est aussi périmé, scaler le
consommateur score-updater.
