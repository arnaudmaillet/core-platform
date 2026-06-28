---
i18n:
  source: ./DOMAIN.md
  source_sha256: 45c95b19e718597f2e3d2550dabb8c8e45a42fa452ce3231f7e3babb6827b059
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `validation` — Contrat de Domaine & Fonctionnel

> La moitié opérationnelle de la validation : middleware + type d'erreur. Il répond à *« où dans le pipeline de commandes les entrées invalides sont-elles rejetées, et comment cet échec est-il façonné pour les clients ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Middleware de validation d'entrée pour le bus de commandes CQRS + le `ValidationError` concret (422) et le catalogue `VAL-xxxx` |
> | **Couche** | `platform` — la moitié opérationnelle de l'abstraction `validate-core` |
> | **Classe de sous-domaine** | **Generic** — validation d'entrée standard ; le levier est le rejet précoce + les erreurs de champ agrégées |
> | **Abstraction(s) primaire(s)** | `ValidationLayer` + `ValidationError` (`validation`) |
> | **Empreinte** | pure (middleware in-process, aucune IO, aucun env) |
> | **Posture en cas d'échec** | **fail-closed pour une entrée invalide** — rejetée avant tout span, enregistrement d'idempotence, ou travail DB |
> | **Dépend de** | `validate-core`, `cqrs`, `error` |
> | **Consommé par** | les composition roots de service (placé le plus à l'extérieur du pipeline de commandes) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `validation` fait autorité dans la flotte pour l'**application de la validation d'entrée** : il
répond à **« comment le contrat abstrait `Validate` est-il transformé en middleware de pipeline qui rejette les
commandes invalides au point le plus précoce, avec un 422 façonné pour le client portant chaque champ en échec ? »**

**Le problème difficile.** La validation doit rejeter *avant* qu'une ressource async ne soit consommée (span,
enregistrement d'idempotence, transaction DB), agréger toutes les erreurs de champ en un aller-retour, et ne rien
coûter pour les commandes valides — tout en gardant le trait abstrait `Validate` dans un crate sans dépendance
pour que `cqrs` puisse l'exiger sans hériter de ce middleware. `validation` est la moitié opérationnelle ;
`validate-core` est l'abstraction.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Définir le trait `Validate` ou `FieldViolation` → ils vivent dans `validate-core` (pour que `cqrs` dépende de
  l'abstraction, pas de ce middleware).
- ❌ Décider les *règles* d'une commande → l'impl `Validate` de chaque commande les possède.
- ❌ Posséder le placement dans le pipeline comme config → c'est une décision de composition root (le plus à l'extérieur).

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Validation layer | Le `CommandLayer` zéro-taille qui appelle `validate()` avant le dispatch | `ValidationLayer`, `ValidationCommandBus` |
| Validation error | Le `AppError` concret (422, `Severity::Low`) enveloppant les violations | `ValidationError` |
| Details map | `field → "VAL-xxxx: message"` pour `ApiErrorResponse.details` | `ValidationError::to_details_map` |
| VAL-xxxx catalogue | Les constantes de code de validation stables | `VAL_1001_REQUIRED` … `VAL_9000_CUSTOM` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `ValidationLayer` | `CommandLayer` zéro-taille | Doit être la couche **la plus à l'extérieur** ; inline `validate()`, éliminé pour les commandes no-op |
| `ValidationCommandBus<S>` | bus décoré | Appelle `payload.validate()` ; `Err` court-circuite avant que le handler ne tourne |
| `ValidationError` | impl `AppError` | `error_code "VAL-0001"`, HTTP 422, `Severity::Low`, retryable false, catégorie `VALIDATION` |
| `to_details_map()` | méthode | `field → code: message` agrégé, directement dans `ApiErrorResponse.details` |
| constantes `VAL_xxxx` | catalogue | Codes stables (`1001 REQUIRED`…`9000 CUSTOM`) |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Le middleware (`ValidationLayer`/`ValidationCommandBus`), le `ValidationError` concret, et le catalogue de
  constantes `VAL-xxxx`.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Le trait `Validate` + `FieldViolation` | `validate-core` | Pour que `cqrs` dépende de l'abstraction sans ce middleware |
| Les règles de validation par commande | l'impl `Validate` de chaque commande | Le middleware est générique sur `C: Command` |
| Le contrat `AppError` / la forme wire | `error` | `ValidationError` l'*implémente* |

**La liste « do-not-depend-on » :** jamais un crate de service/domaine ; il se situe entre `cqrs` (qu'il étend) et
`validate-core`/`error` (auxquels il se conforme).

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | `ValidationLayer` est la couche de commande **la plus à l'extérieur** | composition root (`.layer()` en premier) | travail gaspillé avant rejet |
| I2 | Le rejet a lieu avant le premier `.await` (aucune ressource async consommée) | `ValidationCommandBus` | les commandes invalides coûtent du travail DB/span/idempotence |
| I3 | Tous les champs en échec retournés en un aller-retour | `to_details_map` sur `Vec<FieldViolation>` | erreurs client en morceaux |
| I4 | Les commandes valides incurrent un coût ~zéro (couche zéro-taille, `validate()` inliné) | système de types | overhead inutile |
| I5 | `CqrsError::Handler` enveloppe une erreur type-erased — pas downcastable en `ValidationError` | effacement `cqrs` | inspecter via `error_code()`/`Display`, pas downcast |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Par commande.** `ValidationCommandBus` (le plus à l'extérieur) appelle `envelope.payload.validate()` :
- `Ok(())` → transmettre au pipeline interne (idempotency → tracing → logging → handler).
- `Err(violations)` → envelopper en `ValidationError` → retourner `CqrsError::Handler(ValidationError)` ; le
  handler n'est **jamais** appelé et aucune ressource async n'est touchée.

Pour les commandes utilisant le défaut no-op de `Validate`, le compilateur élimine l'appel entièrement — les
commandes valides ne paient rien.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `validate-core` | amont | Separated Interface | `Validate` + `FieldViolation` | ce que le middleware peut valider |
| `cqrs` | amont | Open-Host (étend) | `ValidationLayer: CommandLayer` | l'intégration au pipeline |
| `error` | amont | Conformist | `ValidationError: AppError` (422) | le mapping d'erreur client |
| composition roots de service | aval | Published Contract | placer `ValidationLayer` le plus à l'extérieur | le rejet d'entrée |

> **Seam de stabilité :** `ValidationError` (`error_code "VAL-0001"`) et le catalogue `VAL-xxxx` sont une API
> publique visible client ; la règle de placement-le-plus-à-l'extérieur de `ValidationLayer` est un contrat.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| `command validation failed — dispatch short-circuited` | `tracing` DEBUG (`command.type`, `violation.count`) | une commande échoue la validation | dashboards filtrés sur `category = VALIDATION` |

DEBUG est intentionnel — les échecs de validation sont un comportement client attendu, pas des incidents. Aucune
mutation d'état externe. Un pic sur `error_code = VAL-0001` peut indiquer un changement cassant côté client ou un
mauvais déploiement.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Placement le plus à l'extérieur — rejeter avant idempotency/tracing/DB | [`README §Architecture`](../README.md) | Accepted |
| Coût zéro pour les commandes valides (couche zéro-taille, `validate()` inliné) | [`README §Architecture`](../README.md) | Accepted |
| Erreurs de champ agrégées via `to_details_map()` | [`README §Architecture`](../README.md) | Accepted |
| Séparation trait/middleware avec `validate-core` (brise le cycle `cqrs`↔`validation`) | [`validate-core README`](../../../foundation/validate-core/README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — validation d'entrée standard ; le levier est le rejet précoce à l'échelle + les
  erreurs de champ agrégées.
- **Stabilité :** contrat stable — `VAL-0001` et le catalogue `VAL-xxxx` sont visibles client.
- **Volatilité :** faible — les nouveaux codes sont des constantes additives.
- **Capacités différées :** un accès downcast structuré au `ValidationError` après l'effacement `cqrs`
  (aujourd'hui : inspecter via `error_code()`/`Display`, ou valider explicitement avant le dispatch).
