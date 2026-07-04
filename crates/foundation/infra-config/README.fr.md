---
i18n:
  source: ./README.md
  source_sha256: d3d525b5d18a799dd2ea54c5286e0beff93bea8d45e3c1a9f72d70178ffbb443
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `infra-config` — Configuration externalisée & hot-reload pour les couches d'infrastructure de la flotte

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — la couche policy / IO qui alimente les crates middleware purs |
> | **Package** | `infra-config` (dir : `crates/foundation/infra-config`) |
> | **Consommé par** | `service-runtime` (charge + surveille) ; les services lisent les profils `[cache]` résolus |
> | **Dépend de** | `notify`, `toml`, `serde`, `arc-swap` |
> | **Stabilité** | évolutif (de nouvelles `[section]`s s'ajoutent au fil du temps) |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`infra-config` est la couche **policy / IO** qui relie les crates middleware purs (p. ex.
[`resilience`](../resilience)) à un paradigme de configuration externalisée. Les crates middleware
possèdent le *mécanisme* (couches Tower, adaptateurs de cache, types filaires serde) ; ce crate possède
tout ce dont le mécanisme ne doit **pas** dépendre : IO fichier + parsing (`infrastructure.toml` → config
typée), validation fail-closed, bindings de flotte (résoudre un nom de dépendance/namespace en un profil
de classe de service), et hot-reload via `notify` avec des swaps `ArcSwap` lock-free.

**Frontière architecturale** — les crates middleware ne lient **aucun** `notify`, `toml`, ni système de
fichiers. Les services dépendent des **deux** : du middleware pour les couches/adaptateurs,
d'`infra-config` pour la provenance des nombres. Il livre quatre sections : `[resilience]`, `[cache]`,
`[traffic]`, `[telemetry]`.

---

## 📐 Architecture & décisions clés

```
infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ InfraRegistry
                                     ▲                          ├─ ResilienceRegistry ─▶ Tower layers
                                notify event                    ├─ CacheRegistry ──────▶ cache adapters
                              spawn_watcher                     ├─ TrafficRegistry ────▶ ingress limiter
                              ──reload──▶ InfraRegistry::apply() └─ TelemetryRegistry ──▶ TelemetrySink (live)
                              (single writer, fail-closed, all-sections-or-nothing)
```

- **Une forme de catalogue, écrite une fois** — chaque section est un catalogue de profils nommés + une
  table de bindings (dépendance → profil, avec un défaut). `catalog::validate_bindings` et `Catalog<L>`
  sont partagés par toutes les sections, donc ajouter un tenant = ajouter une spec + un type live, pas
  réimplémenter la résolution.
- **Deux représentations par section** — un type **Wire** serde plat (`…ProfileSpec`, parsé du TOML) et
  un type **Runtime** (`…Profile`) tenant des handles `Arc<ArcSwap<_>>` que le chemin de données lit à
  chaque appel.
- **Topologie figée au boot, contenu hot-reload** — *quelles* sections/profils existent et *quelle*
  dépendance se lie où est capturé au câblage (un re-binding nécessite un redémarrage). Les *valeurs*
  d'un profil (timeout, TTL) font du hot-reload — c'est le chemin critique en incident.
- **Sûreté du hot-reload** — tous les swaps ont lieu dans **une seule** tâche (pas de course) ; `apply`
  valide *chaque* section présente avant d'en swapper *aucune* (**fail-closed, tout-ou-rien**) ; le
  watcher surveille le **répertoire parent** et non le fichier (les ConfigMaps K8s swappent l'inode du
  symlink `..data`, donc surveiller le chemin de fichier devient sourd après le premier changement) ; les
  rafales d'événements sont fusionnées en un seul reload.

---

## 🔌 API publique & contrat

```rust
// schema.rs — top-level document; new sections are Option<…> for backward compatibility.
impl InfrastructureConfig {
    pub fn from_toml(raw: &str) -> Result<Self, ConfigError>;
    pub fn validate(&self) -> Result<(), ConfigError>;        // every present section
}

// reload.rs — the watcher's target, decoupled from any section's shape.
pub trait Reloadable: Send + Sync + 'static {
    fn reload(&self, raw: &str) -> Result<(), ConfigError>;   // parse + validate + swap, fail-closed
}

// infra.rs — the aggregate registry (drive this in production).
impl InfraRegistry {
    pub fn from_config(InfrastructureConfig) -> Result<Self, ConfigError>;
    pub fn resilience(&self) -> Arc<ResilienceRegistry>;
    pub fn cache(&self) -> Option<Arc<CacheRegistry>>;
    pub fn apply(&self, InfrastructureConfig) -> Result<(), ConfigError>;
}
// impl Reloadable for InfraRegistry (all sections) and for ResilienceRegistry (resilience-only)

// watcher.rs — generic over the target.
pub fn load_from_path(path: &Path) -> Result<InfrastructureConfig, ConfigError>;
pub fn spawn_watcher<R: Reloadable>(path: PathBuf, target: Arc<R>) -> Result<notify::RecommendedWatcher, ConfigError>; // KEEP the guard alive
```

> **Invariants de validation** (avant la résolution *et* chaque hot-swap) : le `default_profile` et les
> cibles de bindings de chaque section doivent référencer un profil défini ; `[resilience]` seuils /
> `half_open_max_calls` / `timeout` > 0 et backoff `max_ms >= base_ms` ; `[cache]` `ttl_secs` > 0.
> `ConfigError` : `Io` · `Toml` · `Watch` · `Validation(String)`.

---

## 📦 Intégration

```toml
[dependencies]
infra-config = { workspace = true }
```

```rust
use std::sync::Arc;
use infra_config::{InfraRegistry, InfrastructureConfig, spawn_watcher};

let registry = Arc::new(InfraRegistry::from_config(
    InfrastructureConfig::from_toml(&std::fs::read_to_string("infrastructure.toml")?)?,
)?);
let _watcher = spawn_watcher("infrastructure.toml".into(), Arc::clone(&registry))?; // keep alive!

// resilience: hot-reloadable client stack from a binding; cache: hand a service its resolved TTLs
let app = profile::app::App::build(backends, registry.cache().expect("[cache] configured")).await?;
```

Voir [`examples/infrastructure.toml`](examples/infrastructure.toml) pour les catalogues + bindings complets.

---

## ⚙️ Configuration & feature flags

Pas de variables d'environnement ni de features cargo — la configuration *est* le fichier
(`infrastructure.toml`, chemin fourni par l'appelant). Le binaire de service (`service-runtime`) charge le
document, lance le watcher, et enregistre la couche traffic + le sink telemetry.

---

## 🧪 Tests

```bash
cargo test   -p infra-config           # unit + a real-filesystem hot-reload integration test
cargo clippy -p infra-config --all-targets
```

Aucun service externe — le test de hot-reload écrit dans un répertoire temporaire et le surveille.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. La config ne se recharge jamais.**
Le guard `RecommendedWatcher` a été droppé. Le lier à une variable longue durée (`let _watcher = …`) ;
le dropper arrête la surveillance.

**2. Reload ignoré après le premier changement (en K8s).**
Quelque chose a surveillé le *chemin* du fichier au lieu du répertoire parent. Ce crate surveille le
parent pour exactement cette raison (les ConfigMaps swappent l'inode du symlink `..data`) — s'assurer que
le parent du montage est accessible.

**3. Erreur `Validation` sur un fichier réputé bon.**
Un invariant a été violé (seuil/TTL à zéro, `max_ms < base_ms`, binding pendant). Lire le message — il
nomme la section et le champ. La config précédente reste live (fail-closed).

**4. Un changement de profil n'a pas été pris malgré un reload.**
C'est un changement de *topologie* (profil/section ajouté/retiré, ou re-binding) — seul le *contenu* des
profils fait du hot-reload. Redémarrer pour appliquer les changements de topologie.
