---
i18n:
  source: ./README.md
  source_sha256: 328030f2dba509f7d0fbe457730d3eb1c151b660160fdc6d39f6fc099b0534a5
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `validation` — Middleware de validation d'entrée inspiré de Tower pour le bus de commandes CQRS

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `platform` — la moitié opérationnelle de la validation (le mécanisme [`validate-core`](../../foundation/validate-core) transformé en middleware) |
> | **Package** | `validation` (dir : `crates/platform/validation`) |
> | **Consommé par** | les racines de composition des services (placé en plus externe du pipeline de commandes) |
> | **Dépend de** | `validate-core`, `cqrs`, `error` |
> | **Stabilité** | contrat stable |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`validation` est la moitié opérationnelle du système de validation d'entrée de la plateforme. Il
transforme le contrat abstrait `Validate` de [`validate-core`](../../foundation/validate-core) en :
`ValidationError` (un `AppError` concret → HTTP 422, `Severity::Low`, portant un `Vec<FieldViolation>` +
un `to_details_map()`), `ValidationLayer` (un `CommandLayer` de taille nulle qui appelle `validate()`
avant dispatch et court-circuite en cas d'échec), et le catalogue stable de constantes `VAL-xxxx`.

**Frontière architecturale** — il possède le *middleware + le type d'erreur* ; le trait `Validate` et
`FieldViolation` vivent dans `validate-core` (pour que `cqrs` dépende de l'abstraction sans le middleware
de ce crate). Les échecs sont rejetés au point le **plus précoce** — avant tout span, enregistrement
d'idempotence ou transaction DB — éliminant le travail gaspillé à l'échelle.

---

## 📐 Architecture & décisions clés

```
Inbound command
   ▼ ValidationCommandBus (OUTERMOST)
   │   envelope.payload.validate()
   │     ├─ Ok(())            ─► inner pipeline
   │     └─ Err(violations)   ─► CqrsError::Handler(ValidationError)   (handler never called)
   ▼ IdempotencyCommandBus ─► TracingCommandBus ─► LoggingCommandBus ─► InMemoryCommandBus ─► handler
```

- **En plus externe à dessein** — rejeter avant idempotence/tracing/DB signifie qu'une commande invalide
  ne consomme aucune ressource async (le rejet a lieu avant le premier `.await`).
- **Coût nul pour les commandes valides** — `ValidationLayer` est de taille nulle ; un unique appel
  `validate()` inliné que le compilateur élimine entièrement pour les commandes au défaut no-op.
- **Erreurs de champ agrégées** — le `Vec<FieldViolation>` + `to_details_map()` renvoie chaque champ en
  échec en un aller-retour, sérialisant directement dans `ApiErrorResponse.details`.

---

## 🔌 API publique & contrat

```rust
pub struct ValidationError { /* violations: Vec<FieldViolation> */ }
impl ValidationError {
    pub fn new(violations: Vec<FieldViolation>) -> Self;     // debug-panics if empty
    pub fn violations(&self) -> &[FieldViolation];
    pub fn to_details_map(&self) -> HashMap<String, String>; // field → "VAL-xxxx: message"
}
// impl AppError: error_code "VAL-0001", http_status 422, severity Low, retryable false, category "VALIDATION"

#[derive(Default, Clone, Copy)] pub struct ValidationLayer;  // zero-size CommandLayer
impl<S> CommandLayer<S> for ValidationLayer { type Service = ValidationCommandBus<S>; /* … */ }

// VAL-xxxx constants:
pub const VAL_1001_REQUIRED: &str = "VAL-1001"; // …1002 LENGTH, 1003 PATTERN, 1004 RANGE, 1005 EMAIL,
pub const VAL_9000_CUSTOM:   &str = "VAL-9000"; //   1006 URL, 1007 ENUM, 1008 SIZE, 1009 UNIQUE, 9000 catch-all
```

> **Contrat :** `ValidationLayer` doit être la couche **la plus externe**. `CqrsError::Handler` enveloppe
> un `BoxedDynAppError` à type effacé — vous **ne pouvez pas** le downcaster vers `ValidationError` ;
> inspecter via `error_code()` (`"VAL-0001"`) et `Display`.

---

## 📦 Intégration

```toml
[dependencies]
validation    = { workspace = true }
validate-core = { workspace = true }   # for Validate + FieldViolation on your command types
```

```rust
use validation::ValidationLayer;
// ValidationLayer FIRST = outermost (MiddlewarePipeline applies layers inside-out).
let bus = MiddlewarePipeline::new(inner)
    .layer(ValidationLayer)
    .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
    .layer(TracingLayer)
    .layer(LoggingLayer)
    .build();

let result = bus.dispatch(Envelope::new(correlation_id, cmd)).await; // invalid ⇒ Err(CqrsError::Handler(_))
```

---

## ⚙️ Configuration & feature flags

Aucun — pas de variables d'environnement ni de features cargo. Le placement dans le pipeline est une
décision de racine de composition, pas de config.

---

## 🔭 Observabilité

Un événement `tracing` : `command validation failed — dispatch short-circuited` (`DEBUG`, champs
`command.type`, `violation.count`). Le DEBUG est intentionnel — les échecs de validation sont un
comportement client attendu, pas des incidents. Filtrer les dashboards sur `category = "VALIDATION"` /
`Severity::Low`.

Alerte suggérée : `error_code = "VAL-0001"` piquant au-dessus de la baseline ⇒ possible changement cassant
côté client ou un mauvais déploiement poussant des données malformées. Surcoût indicatif : ~0 ns valide,
~50–200 ns invalide.

---

## 🧪 Tests

```bash
cargo test   -p validation
cargo clippy -p validate-core -p validation --all-targets
```

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `ValidationLayer` rejette des commandes qui devraient être valides.**
Votre impl `Validate` renvoie `Err(_)` de façon inattendue. Appeler `cmd.validate()` directement dans un
test unitaire et inspecter le `Vec<FieldViolation>`. Cause fréquente : une vérification de longueur avec
`.len()` (octets) vs `.chars().count()` (points de code) sur de l'entrée non-ASCII.

**2. `CqrsError::Handler` renvoyé mais le downcast vers `ValidationError` échoue.**
`Handler` enveloppe un `BoxedDynAppError` à type effacé — pas de downcast après encapsulation. Lire
`error_code()` (`"VAL-0001"`) et `Display` pour les détails de champ ; si vous avez besoin d'accès
structuré, valider explicitement avant dispatch et mapper le résultat vous-même.

**3. Une autre couche intercepte avant `ValidationLayer`.**
`MiddlewarePipeline::layer()` applique de l'intérieur vers l'extérieur — le **premier** appel `.layer()`
est le plus externe. Appeler `.layer(ValidationLayer)` en premier.
