---
i18n:
  source: ./README.md
  source_sha256: 722e806c32d71e252e7f4304cb50d8c6e798d5b9a92d84a910012a217839a57a
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `traffic` — Mécanisme pur de rate-limiting côté serveur : le miroir en entrée de `resilience`

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — mécanisme de rate-limiting en entrée, agnostique du transport |
> | **Package** | `traffic` (dir : `crates/foundation/traffic`) |
> | **Consommé par** | `transport` (extrait une clé, mappe `Throttle` → `RESOURCE_EXHAUSTED`), `infra-config` (parse `[traffic]`) |
> | **Dépend de** | `arc-swap`, `async-trait`, `governor` (GCRA), `serde` (optionnel) |
> | **Stabilité** | évolutif (le mode Distributed arrive en Step 2) |
> | **Feature flags** | `serde` (off par défaut — pur ; `infra-config` l'active) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`traffic` est le mécanisme pur de rate-limiting côté serveur — le **miroir en entrée de `resilience`**.
`resilience` protège un *appelant* d'un aval lent/défaillant (côté client) ; `traffic` protège un
*serveur* d'un trop-plein d'appelants entrants (côté serveur). Les deux partagent le modèle externalisé
catalogue+bindings : `infra-config` parse la section `[traffic]` et résout les bindings en handles
`TrafficProfile` que ce crate produit.

**Frontière architecturale** — délibérément **agnostique du transport** : il possède le limiteur, les
types de config, et un `check(key) -> TrafficDecision`. Pas de `tonic`, pas de `http`, pas de plomberie
d'identité. La couche gRPC qui extrait une clé d'une requête et traduit un `Throttle` en
`RESOURCE_EXHAUSTED` vit dans `transport`, où le couplage tonic/http a sa place.

---

## 📐 Architecture & décisions clés

```
infra-config  ──parses [traffic] (serde on)──►  TrafficProfileSpec ──resolve──►  TrafficProfile
                                                                                      │ check(key)
transport (gRPC): extract key from request ───────────────────────────────────────►  ▼
                  Throttle → RESOURCE_EXHAUSTED        TrafficDecision::Allow | Throttle { retry_after }
```

- **Miroir de `resilience`** — même forme catalogue+bindings, direction opposée. Garder le mécanisme
  symétrique = un seul modèle mental et une seule histoire de config pour l'entrée et la sortie.
- **Pur par défaut** — avec `serde` off, le crate ne tire aucun code serde/derive (même frontière que
  `resilience`) ; `infra-config` active `serde` uniquement là où il doit parser la section.
- **Agnostique du transport** — le crate s'arrête à `check(key) -> TrafficDecision`. L'extraction de clé
  de requête et le mapping `RESOURCE_EXHAUSTED` sont le travail de `transport`, donc `traffic` ne lie
  jamais tonic/http.
- **Localité d'état (Step 1)** — seul `Mode::Local` est imposé : limiteurs `governor` (GCRA) en
  in-process, par-réplica. `Mode::Distributed` est *parsé* pour la compatibilité ascendante mais
  **rejeté par la validation d'`infra-config`** jusqu'à la livraison du backend Redis-lease (Step 2).

---

## 🔌 API publique & contrat

```rust
pub use backend::{Quota, QuotaBackend, QuotaError, DEFAULT_LEASE_MS};
pub use config::{BackendError, Mode, Scope, TrafficConfig, TrafficDecision};
pub use profile::{TrafficProfile, TrafficProfileSpec};

pub enum Mode { Local, Distributed }            // Distributed: parsed, not yet enforced
pub enum Scope { /* keying scope for the limiter */ }
pub enum TrafficDecision { Allow, Throttle { /* retry-after */ } }

impl TrafficProfile {
    pub fn check(&self, key: &str) -> TrafficDecision;   // GCRA decision for this key
    pub fn apply(&self, spec: &TrafficProfileSpec);      // hot-swap config (ArcSwap)
    pub fn prune(&self);                                 // evict idle per-key limiter state
    pub fn key_count(&self) -> usize;
    pub fn enforce(&self) -> bool;
    pub fn scope(&self) -> Scope;
    pub fn mode(&self) -> Mode;
}
```

> **Contrat :** `check(key)` est le chemin chaud — `Allow` ou `Throttle { retry_after }`. `apply`
> hot-swappe la config du profil via `ArcSwap` (piloté par le hot-reload d'`infra-config`). L'état du
> limiteur par-clé croît avec les clés distinctes ; appeler `prune()` périodiquement pour évincer les
> clés inactives.

---

## 📦 Intégration

```toml
[dependencies]
traffic = { workspace = true }                 # add `features = ["serde"]` only to parse config (infra-config does)
```

```rust
use traffic::{TrafficDecision};

// `transport` owns this glue: derive a key from the request, then:
match profile.check(&key) {
    TrafficDecision::Allow => { /* proceed */ }
    TrafficDecision::Throttle { .. } => { /* return RESOURCE_EXHAUSTED with retry-after */ }
}
```

---

## ⚙️ Configuration & feature flags

Pas de variables d'environnement — la config arrive en section `[traffic]` via `infra-config` (catalogue
de `TrafficProfileSpec` + bindings), hot-reloadable via `apply`.

**Feature flags :**
- `serde` — off par défaut (mécanisme pur). `infra-config` l'active pour désérialiser la section
  `[traffic]`. Un service consommant uniquement des handles `TrafficProfile` résolus doit le laisser off.

---

## 🧪 Tests

```bash
cargo test   -p traffic                    # GCRA decisions, hot-swap, pruning
cargo test   -p traffic --features serde   # config (de)serialization
cargo clippy -p traffic --all-targets
```

Bibliothèque pure — aucun service externe (le mode Local est in-process).

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `Mode::Distributed` dans ma config `[traffic]` est rejeté au boot.**
Par conception — seul `Mode::Local` est imposé en Step 1. Distributed est parsé pour la compatibilité
ascendante mais la validation d'`infra-config` le rejette jusqu'à la livraison du backend Redis-lease
(Step 2). Utiliser `Local`.

**2. La mémoire du limiteur croît avec le temps.**
L'état GCRA par-clé accumule une entrée par clé distincte. Appeler `prune()` sur un timer pour évincer
les clés inactives ; vérifier `key_count()` pour dimensionner la cadence.

**3. J'ai ajouté `traffic` et il a tiré serde de façon inattendue / j'ai besoin du parsing mais serde est off.**
La feature `serde` est off par défaut (garde le crate pur). N'activer `features = ["serde"]` que là où
vous parsez la section — les services consommant des handles `TrafficProfile` résolus doivent la laisser off.

**4. Cherchant le mapping `RESOURCE_EXHAUSTED` ou l'extraction de clé de requête — pas ici.**
Ce crate est agnostique du transport et s'arrête à `check(key) -> TrafficDecision`. La glu tonic/http
(extraction de clé, `Throttle` → `RESOURCE_EXHAUSTED`) vit dans `transport`.
