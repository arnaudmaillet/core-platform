---
i18n:
  source: ./README.md
  source_sha256: a93434d6afa43360172d00b68d189be17b8127924ab9bbfc438bd3d320413a53
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `scylla` — Sessions ScyllaDB token-aware avec profils d'exécution, tracing OTel et codes d'erreur stables

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `storage` — crate de capacité d'infrastructure pur (cycle de vie de session, sans schéma) |
> | **Package** | `scylla-storage` (dir : `crates/storage/scylla`) |
> | **Consommé par** | `chat`, `profile`, `social-graph`, `post`, `comment`, `geo-discovery`, `notification`, `timeline` |
> | **Dépend de** | `scylla` 1.5 (driver), `telemetry`, `error` |
> | **Stabilité** | contrat stable |
> | **Feature flags** | aucun propre au crate |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`scylla-storage` est le crate de gestion de sessions ScyllaDB de la flotte. Il fournit une construction de
session token-aware et DC-aware adossée à un cache LRU de statements préparés (`CachingSession`), trois
profils d'exécution intégrés (`Strict` / `Fast` / `Analytical`), un pont de tracing OTel via le
`HistoryListener` du driver, un mapping d'erreur structuré avec des codes `SDB-XXXX` compatibles
`AppError`, et une configuration pilotée par l'environnement + des health checks.

**Frontière architecturale** — c'est un crate de **capacité d'infrastructure pure** : il ne contient
**aucune table de domaine, aucun schéma CQL, aucun modèle applicatif**. Toute interaction ScyllaDB (DDL de
keyspace, DDL de table, requêtes applicatives) appartient aux crates de service qui en dépendent. Énoncer
cette frontière est l'objectif — elle garde la politique de stockage hors de la couche de données.

---

## 📐 Architecture & décisions clés

```
ScyllaConfig::from_env() ─► ScyllaSessionBuilder::build() ─► ScyllaClient {
    session: CachingSession,            // prepared-statement LRU cache
    profiles: ProfileRegistry,          // Strict | Fast | Analytical handles
    history_listener: Arc<dyn HistoryListener>,   // OTel span bridge
}
```

- **`CachingSession`, pas `Session` brut** — le cache prépare-et-réutilise les statements de façon
  transparente, donc les appelants passent des chaînes CQL sans gérer de registre de statements préparés.
  La taille du cache est bornée (`SCYLLA_STATEMENT_CACHE_CAPACITY`).
- **Les profils d'exécution sont de première classe, enregistrés une fois** — `Strict` (mutations),
  `Fast` (lectures sensibles à la latence, avec exécution spéculative), `Analytical` (fond/admin).
  `Strict` est le défaut au niveau session ; un statement choisit un autre profil en attachant son handle.
  Cela pousse la décision d'étagement de cohérence lecture/écriture au site d'appel, où elle a sa place.
- **Le pont OTel est par-statement, pas global** — le driver expose le tracing via un `HistoryListener`
  attaché *par statement*, donc le pont de span est opt-in sur les statements qui comptent plutôt que
  d'envelopper chaque aller-retour.
- **Les erreurs sont mappées vers des codes stables à la frontière** — les variantes `ExecutionError` du
  driver se réduisent en codes `SDB-XXXX` avec une classification retryable/sévérité fixe, donc les
  consommateurs branchent sur un contrat stable au lieu des internes du driver.

---

## 🔌 API publique & contrat

```rust
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder, ScyllaClient, ProfileKind};
use scylla_storage::health::health_check;

pub struct ScyllaConfig { /* … */ }
impl ScyllaConfig { pub fn from_env() -> Self; }

pub struct ScyllaSessionBuilder { /* … */ }
impl ScyllaSessionBuilder {
    pub fn new(config: ScyllaConfig) -> Self;
    pub async fn build(self) -> Result<ScyllaClient, ScyllaStorageError>;
}

pub struct ScyllaClient {
    pub session: CachingSession,
    pub profiles: ProfileRegistry,            // .get(ProfileKind::Strict) -> handle
    pub history_listener: Arc<dyn HistoryListener>,
}

pub enum ProfileKind { Strict, Fast, Analytical }

pub async fn health_check(session: &CachingSession) -> Result<(), ScyllaStorageError>; // system.local probe
```

> **Contrat :** `Strict` est le défaut de session — les statements sans handle de profil explicite
> s'exécutent sous lui. Le crate possède le cycle de vie de session et les profils ; il ne possède ni ne
> valide le schéma. `health_check` sonde `system.local` et est le signal de liveness canonique qu'un
> service câble dans ses `health_probes`.

---

## 📦 Intégration

```toml
[dependencies]
scylla-storage = { workspace = true }
```

```rust
use scylla_storage::{ScyllaConfig, ScyllaSessionBuilder, ProfileKind};
use scylla_storage::health::health_check;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = ScyllaSessionBuilder::new(ScyllaConfig::from_env()).build().await?;
    health_check(&client.session).await?;
    println!("cluster is healthy");
    Ok(())
}
```

### Attacher le listener OTel + un profil par statement

```rust
use std::sync::Arc;
use scylla::observability::history::HistoryListener;
use scylla::statement::unprepared::Statement;

let mut stmt = Statement::new("INSERT INTO feed.events (id, ts) VALUES (?, ?)");
stmt.set_history_listener(Arc::clone(&client.history_listener) as Arc<dyn HistoryListener>);
stmt.set_execution_profile_handle(client.profiles.get(ProfileKind::Strict).clone().into_handle());
client.session.query_unpaged(stmt, (id, ts)).await?;
```

---

## ⚙️ Configuration & feature flags

| Variable | Default | Description |
|---|---|---|
| `SCYLLA_CONTACT_POINTS` | `127.0.0.1:9042` | Comma-separated `host:port` list |
| `SCYLLA_LOCAL_DC` | `datacenter1` | Preferred datacenter for DC-aware load balancing |
| `SCYLLA_KEYSPACE` | _(none)_ | Optional keyspace set on the session |
| `SCYLLA_USERNAME` | _(none)_ | CQL authenticator username |
| `SCYLLA_PASSWORD` | _(none)_ | CQL authenticator password |
| `SCYLLA_COMPRESSION` | `lz4` | Wire compression: `none`, `lz4`, `snappy` |
| `SCYLLA_CONNECT_TIMEOUT_SECS` | `5` | TCP connection timeout in seconds |
| `SCYLLA_REQUEST_TIMEOUT_SECS` | `2` | Default per-request timeout (overridden per profile) |
| `SCYLLA_STATEMENT_CACHE_CAPACITY` | `1000` | Maximum prepared-statement cache entries |

**Profils d'exécution :**

| Profile | Consistency | Speculative | Timeout | Use case |
|---|---|---|---|---|
| `Strict` | `LocalQuorum` | no | 5 s | Mutations — feed writes, follow-graph insertions |
| `Fast` | `LocalOne` | +1 attempt / 50 ms delay | 2 s | Latency-sensitive reads — timelines, feed lookups |
| `Analytical` | `Quorum` | no | 30 s | Background aggregation, admin reads |

**Feature flags :** aucun propre au crate.

---

## 🧯 Modèle d'erreur

`ScyllaStorageError` implémente `error::AppError` (via `From<ExecutionError>`) ; les codes mappent vers
gRPC `Status` / HTTP par le crate partagé `error`.

| Code | Variant | Retryable | Severity |
|---|---|---|---|
| `SDB-1001` | `WriteTimeout` | yes | High |
| `SDB-1002` | `ReadTimeout` | yes | High |
| `SDB-1003` | `Unavailable` | yes | Critical |
| `SDB-1004` | `Overloaded` | yes | High |
| `SDB-1005` | `RateLimitReached` | yes | High |
| `SDB-1006` | `IsBootstrapping` | yes | Medium |
| `SDB-1007` | `ClientTimeout` | yes | High |
| `SDB-2001` | `ConnectionPool` | yes | High |
| `SDB-2002` | `Transport` | yes | High |
| `SDB-3001` | `AuthenticationError` | no | Critical |
| `SDB-3002` | `Unauthorized` | no | Low |
| `SDB-4001` | `AlreadyExists` | no | Low |
| `SDB-5001` | `BadQuery` | no | Critical |
| `SDB-5002` | `QueryInvalid` | no | Medium |
| `SDB-5003` | `WriteFailure` | no | High |
| `SDB-5004` | `ReadFailure` | no | High |
| `SDB-6001` | `SchemaConflict` | no | High |
| `SDB-7001` | `Bootstrap` | no | Critical |
| `SDB-8001` | `Configuration` | no | Critical |
| `SDB-8002` | `ProtocolError` | no | Critical |
| `SDB-9000` | `Unknown` | no | Medium |

---

## 🔭 Observabilité

```
[caller's active span]
  └── scylla.request               ← full query lifecycle
        ├── scylla.attempt         ← primary coordinator round-trip
        └── scylla.speculative_fiber
              └── scylla.attempt   ← speculative backup (Fast profile)
```

Les spans portent `otel.kind = CLIENT`, `db.system = scylladb`, `net.peer.name`, et `net.peer.port` selon
les conventions sémantiques OTel.

---

## 🧪 Tests

```bash
cargo test   -p scylla-storage
cargo clippy -p scylla-storage --all-targets
```

Les tests d'intégration nécessitent un nœud vivant (ils sont `#[ignore]` par défaut) :

```bash
docker run --rm -p 9042:9042 scylladb/scylla --developer-mode=1
SCYLLA_CONTACT_POINTS=127.0.0.1:9042 SCYLLA_LOCAL_DC=datacenter1 \
  cargo test -p scylla-storage -- --include-ignored
```

---

## 🗂️ Organisation des modules

```
src/
├── config/cluster.rs       ScyllaConfig + CompressionKind
├── error/map.rs            ScyllaStorageError + AppError impl + From<ExecutionError>
├── health/check.rs         health_check(session) → system.local probe
├── listener/otel.rs        OtelHistoryListener → tracing span bridge
├── profile/
│   ├── builder.rs          ProfileBuilder fluent API
│   └── registry.rs         ProfileRegistry (Strict / Fast / Analytical)
└── session/builder.rs      ScyllaSessionBuilder → ScyllaClient
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. Le nom du package est `scylla-storage`, mais le crate driver amont est `scylla`.**
Le flag `-p` et la clé `Cargo.toml` sont `scylla-storage` ; `use scylla::…` réfère au **driver**,
`use scylla_storage::…` à ce crate. Les confondre est l'erreur de build la plus courante.

**2. Un statement s'est exécuté sous la mauvaise cohérence.**
Un statement **sans** handle de profil attaché s'exécute sous le défaut de session (`Strict` /
`LocalQuorum`). Les lectures sensibles à la latence doivent attacher explicitement le handle `Fast` —
`set_execution_profile_handle(client.profiles.get(ProfileKind::Fast)…)` — ou elles paient silencieusement
la latence de quorum.

**3. Aucun span n'apparaît pour mes requêtes.**
Le `HistoryListener` OTel est attaché **par statement**, pas globalement. Un statement sans
`set_history_listener(...)` n'émet aucun span `scylla.request`. Attacher le listener sur les statements à
tracer.

**4. Dérive d'API du driver `scylla` 1.5.**
Ce crate cible le driver `scylla` 1.5 ; les API `CachingSession` / `Statement` / `HistoryListener` ont
bougé entre les 1.x. Épingler et monter délibérément — un bump mineur du driver peut déplacer ces surfaces.
