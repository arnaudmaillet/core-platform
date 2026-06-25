---
i18n:
  source: ./README.md
  source_sha256: 229de948e68e2b30c54446639f01f1c826d43d0e825f940ad4d229cdcec8ec4d
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `health` — Abstraction de sondes liveness/readiness, une feuille du graphe pour que le stockage expose des sondes sans le runtime

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — abstraction de sonde feuille-de-graphe (découple le stockage du runtime) |
> | **Package** | `health` (dir : `crates/foundation/health`) |
> | **Consommé par** | `service-runtime` (poll des sondes → santé gRPC), crates de stockage (`scylla`/`redis`/`postgres` exposent des sondes) |
> | **Dépend de** | `async-trait`, `anyhow` |
> | **Stabilité** | contrat stable |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`health` définit l'abstraction de sonde liveness/readiness que la flotte utilise pour piloter le statut
de santé gRPC d'un service. `HealthProbe` est une **feuille de graphe** : les crates de stockage exposent
des sondes prêtes à l'emploi sur leurs clients vivants (ne dépendant que de ce crate fondation, jamais du
runtime), et le runtime poll ces sondes pour piloter la santé — **sans qu'aucun des deux côtés ne dépende
de l'autre**.

**Frontière architecturale** — il ne possède que le *contrat* de sonde. Il n'ouvre pas de connexions, ne
connaît pas gRPC, et n'ordonnance pas le polling ; le runtime fait le polling, les crates de stockage
font la vérification.

---

## 📐 Architecture & décisions clés

```
storage crate (scylla/redis/postgres)         service-runtime
   exposes  impl HealthProbe  ───────────────►  polls .check() each readiness tick
                     ▲                                   │ any Err
              both depend only on `health`               ▼
                                              demote service to NOT_SERVING until a tick clears it
```

- **Feuille de graphe par conception** — placer le trait dans un minuscule crate fondation est ce qui
  laisse un crate de stockage publier une sonde et le runtime la consommer sans arête entre stockage et
  runtime. Déplacer le trait d'un côté ou de l'autre réintroduirait ce couplage.
- **Les sondes doivent être bon marché** — `check()` s'exécute à **chaque** tick de readiness, donc une
  sonde est un ping de joignabilité léger (p. ex. `system.local`, `PING`, `SELECT 1`), jamais une requête
  lourde.
- **Échec → `NOT_SERVING`, auto-effaçant** — toute `Err` de toute sonde rétrograde l'ensemble du service
  jusqu'à ce qu'un tick ultérieur réussisse ; il n'y a pas d'état d'échec persistant à réinitialiser.

---

## 🔌 API publique & contrat

```rust
#[async_trait]
pub trait HealthProbe: Send + Sync + 'static {
    fn name(&self) -> &str;                       // short id for logs, e.g. "scylla" / "redis"
    async fn check(&self) -> anyhow::Result<()>;  // Ok = reachable; any Err demotes to NOT_SERVING
}

/// A HealthProbe backed by an async closure, for a bespoke check not provided by a storage crate.
/// The closure is `Fn` (re-run every tick), typically capturing a cloned client handle.
pub struct FnProbe<F>;
impl<F> FnProbe<F> { pub fn new(name: &'static str, check: F) -> Self; }
```

> **Contrat :** `check()` est poll à chaque tick — garder léger et idempotent. `name()` est pour les logs
> seulement. Les crates de stockage livrent leurs propres impls `HealthProbe` (voir le `health::probe` de
> chaque crate) ; `FnProbe` est l'échappatoire pour tout le reste.

---

## 📦 Intégration

```toml
[dependencies]
health = { workspace = true }
```

```rust
use health::{HealthProbe, FnProbe};

// A bespoke probe over any client handle:
let probe = FnProbe::new("elasticsearch", move || {
    let client = client.clone();
    async move { client.ping().await.map_err(Into::into) }
});

// The runtime collects Vec<Box<dyn HealthProbe>> from the service and polls them each tick.
```

---

## ⚙️ Configuration & feature flags

Aucun — pas de variables d'environnement, pas de features cargo. La cadence de polling et le câblage gRPC
appartiennent à `service-runtime`.

---

## 🧪 Tests

```bash
cargo test   -p health
cargo clippy -p health --all-targets
```

Bibliothèque pure — aucun service externe requis.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. Un service oscille vers `NOT_SERVING` sous charge.**
Le `check()` d'une sonde est trop lourd et expire sur un tick chargé. Les sondes doivent être des pings
de joignabilité légers, pas de vraies requêtes — toute `Err` (timeout compris) rétrograde l'ensemble du
service jusqu'au tick suivant.

**2. Ma closure `FnProbe` ne compile pas / capture une valeur déplacée.**
La closure est `Fn` (ré-invoquée à chaque tick), donc ré-appelable — capturer un handle de client
**cloné** et le `clone()` à nouveau dans le bloc `async move`, comme dans l'exemple.

**3. D'où viennent les sondes `scylla`/`redis`/`postgres` ?**
Chaque crate de stockage expose sa propre `HealthProbe` sur son client (son `health::probe`). Les
utiliser directement ; ne recourir à `FnProbe` que pour les dépendances sans sonde prête à l'emploi.
