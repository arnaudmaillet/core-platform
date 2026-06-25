---
i18n:
  source: ./README.md
  source_sha256: bde3f9a76dceba55c2a8953316c95b2cc4c9a110eb8c8ec5deef3e8b52fc5328
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `validate-core` — Abstraction de validation sans dépendance, partagée par `cqrs` et `validation`

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — la frontière d'abstraction de validation (Separated Interface) |
> | **Package** | `validate-core` (dir : `crates/foundation/validate-core`) |
> | **Consommé par** | `cqrs` (supertrait de `Command`), `validation` (middleware + types d'erreur) |
> | **Dépend de** | **rien** — zéro dépendance est une contrainte dure et imposée |
> | **Stabilité** | contrat stable |
> | **Feature flags** | aucun (l'ensemble vide impose le zéro-dépendance) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`validate-core` est la **frontière d'abstraction de validation** à l'échelle du workspace. Il définit
exactement deux items publics : `FieldViolation` (un échec de niveau champ portant un code `VAL-xxxx`
stable, un chemin de champ en notation pointée et un message) et `Validate` (un trait que tout type
implémente pour exprimer ses propres vérifications d'invariants, renvoyant `Ok(())` ou
`Err(Vec<FieldViolation>)`).

**Frontière architecturale** — il existe pour que `cqrs` et `validation` pointent tous deux vers une
abstraction partagée **sans dépendre l'un de l'autre**. Il ne doit jamais acquérir de dépendance, ni
tirer de middleware, de types HTTP, ou de machinerie de framework d'erreur.

---

## 📐 Architecture & décisions clés

```
       validate-core          (zero deps — the abstraction)
        ▲            ▲
        │            │
      cqrs       validation
  (Validate as    (ValidationLayer + ValidationError + VAL-xxxx codes)
   Command supertrait)
```

- **Separated Interface, pas un module dans `validation`** — si `Validate` vivait dans `validation`,
  `cqrs` dépendrait de `validation` et hériterait de sa pile middleware/HTTP/erreur, violant le SRP. Un
  troisième crate fin laisse les deux côtés du graphe converger sans couplage.
- **Agrégation, pas court-circuit** — `validate()` doit collecter **toutes** les violations avant de
  renvoyer. Un formulaire avec trois mauvais champs renvoie trois codes en un aller-retour, pas une
  erreur par soumission.
- **Champs `&'static str`** — `field` et `code` sont statiques, donc une violation n'alloue rien sur le
  chemin chaud de validation ; seul le `Vec` alloue, et seulement quand il *y a* une violation.

---

## 🔌 API publique & contrat

```rust
pub struct FieldViolation {
    pub field:   &'static str,   // dot-notation path, e.g. "user.email"
    pub code:    &'static str,   // stable VAL-xxxx code, e.g. "VAL-1001"
    pub message: String,
}
impl FieldViolation { pub fn new(field: &'static str, code: &'static str, message: impl Into<String>) -> Self; }

pub trait Validate {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> { Ok(()) }   // default no-op — override when constrained
}
```

> **Contrat :** `Validate` est un supertrait de `cqrs::Command`, donc toute commande le satisfait via le
> défaut sauf surcharge. Un `Err(violations)` renvoyé doit être **non vide** par convention
> (`ValidationError::new` dans le crate `validation` le `debug_assert!`). `field`/`code` doivent rester
> `&'static str` — jamais `String`.

---

## 📦 Intégration

```toml
[dependencies]
validate-core = { workspace = true }
```

```rust
use validate_core::{FieldViolation, Validate};

impl Validate for CreateUserCommand {
    fn validate(&self) -> Result<(), Vec<FieldViolation>> {
        let mut v = Vec::new();
        if self.username.is_empty()   { v.push(FieldViolation::new("username", "VAL-1001", "must not be empty")); }
        if self.username.len() > 32   { v.push(FieldViolation::new("username", "VAL-1002", "must be at most 32 characters")); }
        if self.age < 13              { v.push(FieldViolation::new("age", "VAL-1004", "must be at least 13")); }
        if v.is_empty() { Ok(()) } else { Err(v) }
    }
}

// A command with no constraints uses the no-op default:
impl Validate for PingCommand {}
```

---

## ⚙️ Configuration & feature flags

Aucun. Pas de config runtime, pas de variables d'environnement, pas de features cargo — l'ensemble de
features vide est ce qui *impose* la garantie zéro-dépendance.

---

## 🧪 Tests

```bash
cargo test   -p validate-core          # doc-tests only (no unit tests by design)
cargo clippy -p validate-core --all-targets
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `Validate is not implemented for MyCommand` (et `Command` l'exige).**
Tout `cqrs::Command` doit aussi implémenter `Validate`. Ajouter `impl Validate for MyCommand {}` pour le
défaut no-op, ou une vraie impl si la commande transporte des données utilisateur.

**2. `ValidationError::new` a paniqué en debug alors que mon `violations` était vide.**
Il `debug_assert!`e que le vec est non vide. `validate()` ne doit renvoyer `Err(v)` que quand `v` a ≥ 1
violation — protéger avec `if v.is_empty() { Ok(()) } else { Err(v) }`.

**3. Une PR a ajouté une dépendance / un import `std::collections` et la CI/revue l'a refusé.**
Par conception : `[dependencies]` doit rester vide et aucun `use` de `std::collections`, d'async, ou de
types de framework d'erreur n'est autorisé. La contrainte zéro-dép est toute la raison d'être du crate.
