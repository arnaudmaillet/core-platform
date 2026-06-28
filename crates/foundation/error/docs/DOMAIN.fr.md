---
i18n:
  source: ./DOMAIN.md
  source_sha256: 1847c3ce0897c5ea218ee90badf44cbef485c1ac13aa94ca5e8dbf507ba10067
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `error` — Contrat de Domaine & Fonctionnel

> Le contrat d'erreur distribuée : un trait, une forme wire, zéro fuite — il répond à *« comment chaque service expose-t-il ses propres erreurs de façon uniforme, observable, et sans fuiter d'internes aux clients ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Le contrat d'erreur à l'échelle du workspace : un trait, un vocabulaire de sévérité, une enveloppe typée, et une forme wire sûre pour les clients |
> | **Couche** | `foundation` — un crate de contrat quasi-racine (presque tout en dépend) |
> | **Classe de sous-domaine** | **Generic** — un contrat transversal ; son levier est l'uniformité + l'anti-fuite |
> | **Abstraction(s) primaire(s)** | `AppError` + `DistributedError<E>` (`error::traits`, `error::context`) |
> | **Empreinte** | pure (aucune IO, aucun état, aucun thread de fond) ; `axum` est une dev-dependency uniquement |
> | **Posture en cas d'échec** | N/A — il *décrit* les échecs ; il ne peut jamais en *causer* un (aucune IO, aucun état) |
> | **Dépend de** | `thiserror`, `tracing`, `http`, `uuid`, `chrono` |
> | **Consommé par** | `cqrs`, `transport`, `resilience`, `validation`, et chaque crate de service |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `error` fait autorité dans la flotte pour le **contrat d'erreur** : il répond à
**« comment un service expose-t-il son propre enum d'erreur typé pour que le logging, le paging, la
classification de retry, et le JSON client soient uniformes à travers chaque service — et que les identifiants
de trace n'atteignent jamais un client ? »**

**Le problème difficile.** Chaque service a besoin de son *propre* enum d'erreur (pour pattern-matcher), mais
la plateforme a besoin d'*une* sortie observable et sûre pour les clients. `error` réconcilie cela avec une
enveloppe typée (`DistributedError<E>` garde le type concret — pas de `Box<dyn Error>`) et une frontière de
divulgation stricte (`trace_id`/`span_id` vivent sur le contexte pour les logs mais sont structurellement
absents de la forme wire client).

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Définir une erreur métier/domaine quelconque → chaque service possède son enum ; ce crate ne possède que le contrat.
- ❌ Décider le comportement de retry → il expose `is_retryable()` ; `resilience` y agit.
- ❌ Posséder la config de logging (format, sampling, alerting) → c'est le bootstrap du binaire consommateur.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| App error | Le contrat qu'un service implémente sur son enum | `AppError` |
| Error code | Code stable, visible client, lisible machine (ex. `AUTH_TOKEN_EXPIRED`) | `AppError::error_code` |
| Severity | Le vocabulaire d'urgence unifié pilotant paging/niveau de log | `Severity` |
| Context | Métadonnées de requête/trace enveloppant une erreur | `ErrorContext` |
| Distributed error | L'enveloppe typée, préservant le type | `DistributedError<E>` |
| Api error response | Le JSON client agnostique au framework et sans fuite | `ApiErrorResponse`, `into_api_response` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `AppError` | trait (seam) | Seuls `error_code()` + `http_status()` sont requis ; le reste a des défauts sûrs en production |
| `IntoApiResponse` | trait blanket | Un seul `to_api_response` canonique pour chaque `AppError` ; ne doit **pas** être surchargé |
| `DistributedError<E>` | enveloppe typée | Préserve le `E` concret de bout en bout (pas d'effacement) ; `.log()` émet trace+span ids |
| `ErrorContext` | type valeur | Porte `trace_id`/`span_id` — présent dans les logs, **absent** de la forme wire client |
| `ApiErrorResponse` | forme wire | La seule struct envoyée aux clients ; ne contient aucun trace/span id |
| `Severity` | enum | `Ord` comme `Critical < High < Medium < Low < Info` (« plus d'urgence = valeur plus basse ») |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Les quatre piliers — contrat (`AppError`/`IntoApiResponse`), vocabulaire (`Severity`), contexte
  (`ErrorContext`/`DistributedError`), et format wire (`ApiErrorResponse`). La frontière anti-fuite est imposée
  *structurellement* ici.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Tout enum d'erreur de domaine concret | chaque crate de service | Ce crate est contrat-seulement, agnostique au consommateur |
| Format de logging / sampling / routage d'alertes | le binaire consommateur (`telemetry`) | La policy opérationnelle n'est pas le travail du contrat |
| Le glue framework `axum`/HTTP | le service (newtype) | `axum` reste une dev-dependency ; la forme wire est agnostique au framework |

**La liste « do-not-depend-on » :** jamais un crate de service, jamais `axum`/`tonic` hors dev-deps, jamais
d'IO réseau ou d'état — pour que `error` ne puisse jamais être la cause d'une panne en cascade.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | `error_code` est une API publique stable (clients/dashboards s'y indexent) | convention de contrat | un renommage est cassant, nécessite une migration |
| I2 | `trace_id`/`span_id` n'atteignent jamais un client | système de types (`ApiErrorResponse` les omet) + `into_api_response` les retire | fuite structurelle seulement si vous sérialisez `ErrorContext` directement |
| I3 | Le type d'erreur concret est préservé de bout en bout (pas de `Box<dyn Error>`) | générique `DistributedError<E>` | perte du pattern-matching |
| I4 | Les nouvelles méthodes `AppError` doivent porter un défaut sûr en production | définition du trait | casse les implémenteurs existants |
| I5 | L'impl blanket `IntoApiResponse` n'est pas surchargée | impl blanket | formes client divergentes |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Chemin d'erreur.** L'enum d'un service implémente `AppError` ; la frontière l'enveloppe en
`DistributedError<E>` avec un `ErrorContext`. `.log()` émet un événement `tracing` à `severity().log_level()`
(avec `trace_id`/`span_id` *à l'intérieur* du log). Le body client est construit via `into_api_response(&err)`
— qui retire les trace/span ids — et retourné avec `http_status()`. Aucun heap sur le chemin chaud : les
méthodes `AppError` retournent `&'static str` et l'enveloppe est allouée sur la pile jusqu'au retour (les
services peuvent la `Box` sur les chemins sensibles à la latence pour garder le `Result` petit).

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| chaque crate de service | aval | Published Contract | `impl AppError` sur leur enum | sortie d'erreur uniforme à l'échelle de la flotte |
| `cqrs` | aval | Conformist | `CqrsError` implémente/délègue `AppError` | le mapping d'erreur du bus |
| `resilience` | aval | Conformist | `AppError::is_retryable` | la classification de retry |
| `validation` | aval | Conformist | `ValidationError: AppError` (422) | le mapping d'erreur de validation |
| `transport` | aval | Conformist | `grpc_severity` / mapping d'erreur | la sévérité d'erreur gRPC |

> **Seam de stabilité :** `AppError` (surtout `error_code`) et `ApiErrorResponse` sont le contrat auquel chaque
> consommateur se lie ; les changements de `error_code` sont cassants pour les clients.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| log d'erreur structuré | `tracing` (niveau = sévérité) | `DistributedError::log()` est appelé | pipeline de logs ; corréler par `request_id` (visible client) ou `trace_id`+`span_id` (logs seulement) |

Aucune métrique, aucun store externe. Le seul effet de bord est l'événement `tracing` optionnel.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Enveloppe typée (`DistributedError<E>`), pas d'effacement `Box<dyn Error>` | [`README §Architecture`](../README.md) | Accepted |
| Divulgation à deux niveaux — seuls `error_code`+`http_status` requis, le reste par défaut | [`README §Architecture`](../README.md) | Accepted |
| Fuite rendue structurellement impossible (trace/span absents de la forme wire) | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — un contrat transversal ; le levier est l'uniformité, l'observabilité, et
  l'anti-fuite, pas la valeur métier.
- **Stabilité :** contrat stable — `error_code` est traité comme une API publique.
- **Volatilité :** faible — les nouvelles méthodes `AppError` sont additives (par défaut) ; la forme wire est stabilisée.
- **Capacités différées :** aucune structurelle ; des payloads de détails plus riches se sérialisent via
  `ApiErrorResponse.details` au besoin.
