---
i18n:
  source: ./DOMAIN.md
  source_sha256: 2f46c67d540bbe6e0ed3a49ee439585f826eca5dfd7f3751ef33ad7fbff0e603
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `validate-core` — Contrat de Domaine & Fonctionnel

> L'abstraction de validation zéro-dépendance : une Separated Interface répondant à *« comment `cqrs` et `validation` peuvent-ils partager un contrat de validation sans dépendre l'un de l'autre ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | La frontière d'abstraction de validation — le trait `Validate` + la primitive `FieldViolation` |
> | **Couche** | `foundation` — une vraie feuille du graphe ; **zéro dépendance** par contrainte dure |
> | **Classe de sous-domaine** | **Generic** — une Separated Interface ; toute sa valeur est l'inversion de dépendance qu'elle permet |
> | **Abstraction(s) primaire(s)** | `Validate` + `FieldViolation` (`validate_core`) |
> | **Empreinte** | pure (zéro dep, aucune IO, aucun état) — le feature set vide *impose* le zéro-dep |
> | **Posture en cas d'échec** | N/A — il ne fait que *décrire* les échecs niveau champ |
> | **Dépend de** | **rien** (imposé) |
> | **Consommé par** | `cqrs` (comme supertrait de `Command`), `validation` (middleware + type d'erreur) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `validate-core` fait autorité dans la flotte pour l'**abstraction de validation** : il répond à
**« que signifie pour un type de se valider lui-même, et comment `cqrs` et `validation` s'accordent là-dessus
sans se coupler l'un à l'autre ? »**

**Le problème difficile.** `cqrs::Command` a besoin de `Validate` comme supertrait, et `validation` fournit le
middleware + le type d'erreur concret — mais si `Validate` vivait dans `validation`, alors `cqrs` hériterait de
toute la stack middleware/HTTP/erreur de `validation`. Un troisième crate sans dépendance permet aux deux côtés
de pointer *vers l'intérieur* vers la même abstraction. La contrainte zéro-dépendance n'est pas accessoire —
c'est la raison d'être entière du crate.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Fournir le *middleware* de validation ou le mapping `AppError`/HTTP → relève de `validation`.
- ❌ Tirer un framework, runtime async, ou `std::collections` → cela défairait l'inversion.
- ❌ Décider *quand* la validation tourne dans le pipeline → c'est la décision de composition de `validation`.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Field violation | Un échec niveau champ unique (chemin du champ + code stable + message) | `FieldViolation` |
| Validation code | Un code `VAL-xxxx` stable, lisible machine | `FieldViolation::code` (`&'static str`) |
| Validate | Le trait qu'un type implémente pour exprimer ses propres vérifications d'invariants | `Validate` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `FieldViolation` | type valeur | `field`/`code` sont `&'static str` (aucun heap sur le chemin chaud) ; `message` est la seule allocation |
| `Validate` | trait (seam) | `validate()` doit **agréger** toutes les violations, jamais court-circuiter ; le défaut est un no-op `Ok(())` |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Exactement deux items publics — `FieldViolation` et `Validate`. Rien d'autre, par conception.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Le middleware `ValidationLayer` + `ValidationError` | `validation` | La moitié opérationnelle dépend *vers le haut* de cette abstraction |
| Le câblage du supertrait `Command` | `cqrs` | `cqrs` dépend *vers le haut* de cette abstraction, pas de `validation` |

**La liste « do-not-depend-on » :** **tout.** `[dependencies]` doit rester vide ; aucun `use` de
`std::collections`, async, ou framework d'erreur. La CI/revue repousse toute arête ajoutée — la garantie
zéro-dep est le contrat.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | Le crate a **zéro** dépendance | `[dependencies]` vide + feature set vide | rejet CI/revue |
| I2 | `validate()` agrège *toutes* les violations (pas de court-circuit) | convention de contrat | rapport d'erreur partiel |
| I3 | Un `Err(violations)` retourné est non-vide | convention (`validation::ValidationError::new` debug-asserte) | debug panic en aval |
| I4 | `field`/`code` restent `&'static str` (jamais `String`) | définition de type | heap sur le chemin de validation |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

N/A — crate d'abstraction pur, aucun flot de contrôle runtime. Le `validate()` d'un type tourne de façon
synchrone là où l'appelant l'invoque (en pratique, dans `validation::ValidationLayer`, avant le premier
`.await`). Il n'y a aucun état, aucun cycle de vie, aucun travail de fond.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `cqrs` | aval | Separated Interface | `Validate` comme supertrait de `Command` | la validabilité de chaque commande |
| `validation` | aval | Separated Interface | `ValidationError` enveloppe `Vec<FieldViolation>` | le middleware + le mapping d'erreur |

> **Seam de stabilité :** `Validate` et `FieldViolation` sont l'API publique entière ; un changement se propage
> aux deux consommateurs à la fois. La forme de convergence (les deux pointent vers l'intérieur, aucun vers
> l'autre) est la garantie architecturale.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

N/A — pur, zéro-dépendance. Il n'émet rien (même pas de `tracing` — ce serait une dépendance). Le log
« validation failed » est émis par `validation`.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Separated Interface dans un troisième crate (pas un module dans `validation`) pour briser le cycle `cqrs`↔`validation` | [`README §Architecture`](../README.md) | Accepted |
| Agrégation plutôt que court-circuit (tous les champs en échec en un aller-retour) | [`README §Architecture`](../README.md) | Accepted |
| Zéro dépendance comme contrainte imposée et porteuse | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — une Separated Interface ; son levier est purement le graphe de dépendances
  qu'elle rend possible.
- **Stabilité :** contrat stable — les deux items sont stabilisés.
- **Volatilité :** minimale — toute croissance risque la garantie zéro-dep, donc la croissance est activement résistée.
- **Capacités différées :** aucune ; une validation plus riche (async, cross-field) vivrait dans `validation`,
  jamais ici.
