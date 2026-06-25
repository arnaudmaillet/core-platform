---
i18n:
  source: ./README.md
  source_sha256: 71c9ca67183e326711ffc0b3e41234b3090be2395672f78298b47406ad516c5c
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `traffic-redis` — Backend distribué à lease Redis pour le rate limiter `traffic`

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `platform` — le `QuotaBackend` qui rend les profils `traffic` fleet-global (Step 2) |
> | **Package** | `traffic-redis` (dir : `crates/platform/traffic-redis`) |
> | **Consommé par** | `transport` (câblé comme `QuotaBackend` des profils traffic `distributed`) |
> | **Dépend de** | `traffic`, `redis-storage`, `fred` (`i-scripts`), `async-trait`, `dashmap` |
> | **Stabilité** | évolutif |
> | **Feature flags** | `integration-traffic-redis` (test Redis live ; off par défaut) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`traffic-redis` implémente [`traffic::QuotaBackend`](../../foundation/traffic) pour que les profils
`distributed` imposent un budget **fleet-global** **sans aller-retour Redis par requête** : chaque
réplica lease un morceau du budget global par-fenêtre et le sert localement, ne traversant vers Redis que
quand son morceau est épuisé (ou pour découvrir que la fenêtre est entièrement dépensée).

**Frontière architecturale** — il ne fournit que le *backend de quota* distribué. Le mécanisme de
limiteur, les types de config, et la décision `check()` vivent dans
[`traffic`](../../foundation/traffic) ; la glu gRPC qui câble ce backend et mappe les décisions vit dans
[`transport`](../transport). L'IO backend est amorti sur `burst` requêtes par clé par réplica ; une
fenêtre entièrement dépensée est cachée localement pour qu'un flot de requêtes hors-budget ne martèle pas
Redis.

---

## 📐 Architecture & décisions clés

```
traffic::QuotaBackend (trait, in `traffic`)
  └─ RedisLeaseBackend
       ├─ LeaseBook       — local per-key lease cache + windowed-budget algorithm (PURE)
       └─ ClaimSource     — atomic "lease N tokens" seam
            └─ RedisClaimSource — one Lua script (single key → cluster-slot-safe)
```

- **Lease-un-morceau, pas check-par-requête** — tout le but : un réplica réclame `burst` jetons d'un coup
  et les sert localement, donc le chemin chaud touche rarement le réseau. L'IO backend scale avec les
  refills, pas les requêtes.
- **Algorithme pur derrière une couture `ClaimSource`** — `LeaseBook` (la logique de budget fenêtré) est
  agnostique du transport et de Redis, et unit-testé contre un `ClaimSource` **en mémoire** ;
  `RedisClaimSource` est la fine implémentation live. C'est ce qui garde la suite unitaire hermétique.
- **Un script Lua, une clé** — la réclamation atomique est un script Lua mono-clé, donc
  **cluster-slot-safe** (pas de `CROSSSLOT`).
- **Fail-soft par policy** — un échec de réclamation surface en `traffic::QuotaError` ; `transport` le
  mappe sur la policy `on_backend_error` du profil (dégrader vers le limiteur local, ou rejeter). Les
  requêtes servies depuis un lease local existant ne touchent jamais Redis, donc un hoquet Redis n'affecte
  que les refills.

---

## 🔌 API publique & contrat

```rust
pub use claim::ClaimSource;
pub use lease::{window_budget, LeaseBook};
pub use redis::{RedisClaimSource, RedisLeaseBackend};

pub trait ClaimSource: Send + Sync { /* atomic "lease N tokens for key in window" */ }

pub struct LeaseBook;                                  // pure local lease cache + windowed-budget algorithm
impl LeaseBook {
    pub fn new() -> Self;
    pub async fn check<C: ClaimSource>(&self, /* key, quota, now, source */) -> /* decision */;
    pub fn prune(&self, now_ms: u64, lease_ms: u64);   // evict idle per-key leases
    pub fn tracked_keys(&self) -> usize;
}
pub fn window_budget(rps: u32, lease_ms: u64) -> u64;  // tokens available in one lease window

pub struct RedisClaimSource;  impl { pub fn new(client: RedisClient) -> Self; }            // the one Lua script
pub struct RedisLeaseBackend; impl traffic::QuotaBackend for RedisLeaseBackend { /* … */ }
impl RedisLeaseBackend { pub fn new(client: RedisClient) -> Self; pub fn prune(&self, lease_ms: u64); pub fn tracked_keys(&self) -> usize; }
```

> **Contrat :** `RedisLeaseBackend` est câblé dans le serveur gRPC comme `Arc<dyn traffic::QuotaBackend>`
> (voir `transport`). Exécuter son `prune(lease_ms)` sur un timer (le `lease_ms` doit correspondre à la
> fenêtre de lease des profils) pour borner la mémoire par-clé. Seuls les profils `distributed`
> consultent le backend ; les profils `local` ne le touchent jamais.

---

## 📦 Intégration

```toml
[dependencies]
traffic-redis = { workspace = true }
```

```rust
// transport server wiring (distributed mode):
let backend = Arc::new(traffic_redis::RedisLeaseBackend::new(redis_client));
builder = builder
    .with_traffic(Arc::clone(&traffic))
    .with_traffic_backend(Arc::clone(&backend) as Arc<dyn traffic::QuotaBackend>);

let lease_ms = 1_000; // match the profiles' lease window
tokio::spawn(async move {
    let mut tick = tokio::time::interval(Duration::from_secs(60));
    loop { tick.tick().await; backend.prune(lease_ms); }
});
```

---

## ⚙️ Configuration & feature flags

Pas de variables d'environnement propres — il prend un `RedisClient` (configuré via `redis-storage`) et
est piloté par les profils `[traffic]` `distributed` résolus par `infra-config`.

**Feature flags :** `integration-traffic-redis` — gate le test Redis live (Docker requis). Off par défaut
pour que la suite unitaire (qui exerce l'algorithme de lease contre un `ClaimSource` en mémoire) reste
hermétique. `fred` est compilé avec `i-scripts` pour le script Lua de réclamation.

---

## 🧪 Tests

```bash
cargo test   -p traffic-redis                                   # hermetic — LeaseBook vs in-memory ClaimSource
cargo test   -p traffic-redis --features integration-traffic-redis   # live Redis (Docker)
cargo clippy -p traffic-redis --all-targets
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. Un profil `distributed` se comporte comme une limite par-réplica.**
Aucun backend n'est câblé — les profils `distributed` **dégradent vers le governor local** quand aucun
`QuotaBackend` n'est fourni. Câbler `RedisLeaseBackend` via `with_traffic_backend(...)` au boot.

**2. La mémoire par-clé croît avec le temps.**
`LeaseBook` conserve une entrée par clé active. Appeler `RedisLeaseBackend::prune(lease_ms)` sur un timer
(correspondant à la fenêtre de lease) ; vérifier `tracked_keys()` pour dimensionner la cadence.

**3. La limite effective est plus lâche/serrée que le rps configuré.**
`window_budget(rps, lease_ms)` fixe combien de jetons un réplica réclame par lease — le `lease_ms` passé à
`prune` doit correspondre à la fenêtre de lease du profil, sinon la comptabilité dérive. Les garder égaux.

**4. Erreur `CROSSSLOT` sur un Redis Cluster.**
Ne devrait pas arriver — la réclamation est un script Lua mono-clé (slot-safe) par conception. Si vous la
voyez, un appelant a construit une opération multi-clés hors de `RedisClaimSource` ; garder la réclamation
au seul script.
