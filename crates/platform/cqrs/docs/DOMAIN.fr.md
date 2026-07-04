---
i18n:
  source: ./DOMAIN.md
  source_sha256: 3948dc98bcebed2b9ac18fcf3795c3ef0fa3c7b5a4bdc27556b0350f8d631166
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `cqrs` — Contrat de Domaine & Fonctionnel

> Le bus Command/Query in-process : une couche de dispatch zéro-overhead répondant à *« quel handler unique possède cette opération, et comment son contexte causal se propage-t-il ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Dispatch applicatif : un Command Bus + Query Bus statique et in-process avec un pipeline de middleware typé |
> | **Couche** | `platform` — la couche de dispatch applicatif entre handlers de transport et handlers de domaine |
> | **Classe de sous-domaine** | **Generic** — un bus CQRS ; le levier est le chemin chaud sans dispatch dynamique + middleware uniforme |
> | **Abstraction(s) primaire(s)** | `Command`/`Query` + `Envelope<T>` + `CommandBus`/`QueryBus` (`cqrs`) |
> | **Empreinte** | pure (in-process, aucune file, aucun réseau, aucun env) |
> | **Posture en cas d'échec** | N/A — il route ; les échecs sont ceux du handler, surfacés en `CqrsError` |
> | **Dépend de** | `error`, `validate-core`, `uuid`, `chrono`, `dashmap`, `tracing` |
> | **Consommé par** | chaque service (bus command/query) ; `validation` & `auth-context` l'étendent |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `cqrs` fait autorité dans la flotte pour le **dispatch applicatif** : il répond à
**« quel handler enregistré unique possède cette commande/query, et comment `correlation_id`/`causation_id`
voyagent-ils de bout en bout ? »** — sans dispatch dynamique sur le chemin chaud.

**Le problème difficile.** Un bus qui route par type implique d'ordinaire de la réflexion ou une vtable à
chaque appel. `cqrs` route par `TypeId` (un `HashMap::get` + un `Box::new` pour franchir une frontière
d'effacement *scellée*), garde les handlers monomorphisés via RPIT natif (pas de `async_trait`), et impose la
séparation écriture/lecture dans le système de types — les commandes mutent et retournent `()`, les queries
retournent des données et ne portent aucun effet de bord.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Être une file de messages / un bus réseau → il est purement in-process.
- ❌ Posséder la *règle* de validation → `Validate` vit dans `validate-core` ; le middleware dans `validation`.
- ❌ Persister l'idempotence durablement → le `InMemoryIdempotencyStore` fourni est non borné ; swappez un store TTL.
- ❌ Posséder l'init tracing/télémétrie → il émet spans/logs mais `telemetry::init()` est un prérequis.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Command / Query | Une écriture (retourne `()`) / une lecture (retourne des données typées) | `Command`, `Query` |
| Handler | Le propriétaire unique d'un type command/query | `CommandHandler`, `QueryHandler` |
| Envelope | Le porteur de message avec la chaîne causale | `Envelope<T>` (`message_id`/`correlation_id`/`causation_id`) |
| Bus | Le dispatcheur routant vers un handler | `CommandBus`, `QueryBus`, `InMemoryCommandBus` |
| Layer / pipeline | Middleware enveloppant le dispatch | `CommandLayer`, `MiddlewarePipeline`, `Tracing`/`Logging`/`Idempotency` |
| Cqrs error | L'enveloppe d'erreur niveau dispatch | `CqrsError` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `Envelope<T>` | porteur de message | `new` démarre une chaîne causale fraîche ; `new_caused_by` hérite la corrélation + fixe la causation |
| `Command` / `Query` | trait (seam) | Séparation écriture/lecture ; supertrait `Command: validate_core::Validate` |
| `CommandBus` / `QueryBus` | trait | **Non object-safe** (`dispatch<C>` générique) — tenir le bus concret ou son `Arc` |
| `CqrsError` | enveloppe d'erreur | `Handler(e)` délègue `error_code`/`http_status`/`severity` à l'erreur originale du handler |
| `IdempotencyStore` | trait (seam) | `mark_processed` ne tourne que sur `Ok` → les handlers échoués restent retentables |
| `CommandLayer`/`QueryLayer` | trait (seam) | Middleware custom ; le bus final est un type concret décoré |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Le mécanisme de dispatch, le modèle enveloppe + chaîne causale, la forme du pipeline de middleware, et les
  couches fournies (`Tracing`/`Logging`/`Idempotency`). Le seul `dyn` est scellé dans des bridges effacés `pub(crate)`.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Le trait `Validate` | `validate-core` | `cqrs` dépend *vers le haut* de l'abstraction, pas de `validation` |
| Le middleware de validation (`ValidationLayer`) | `validation` | Il étend `cqrs` via `CommandLayer` |
| L'injection d'identité dans les enveloppes | `auth-context` (`cqrs-integration`) | Il étend `cqrs`, pas l'inverse |
| L'idempotence durable (Redis TTL) | le service | Le store in-memory est un défaut à swapper |

**La liste « do-not-depend-on » :** jamais `tonic`/`rdkafka`/réseau/env — il reste un bus in-process pur.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | Un type command/query a **exactement un** handler | `register` du builder (eager) | `CqrsError::DuplicateRegistration` au build |
| I2 | Dispatcher un type non enregistré échoue vite | dispatch `InMemoryCommandBus` | `CqrsError::HandlerNotFound` |
| I3 | Aucun dispatch dynamique sur le chemin chaud (routage TypeId, RPIT natif) | système de types | `BoxFuture` uniquement dans le bridge scellé |
| I4 | L'idempotence ne marque que sur `Ok` (les échecs restent retentables) | `IdempotencyLayer` | — |
| I5 | Les queries ne portent aucun effet de bord (aucun chemin d'écriture via `QueryBus`) | système de types (séparation écriture/lecture) | — |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Build (démarrage).** `CommandBusBuilder::register::<C, _>(handler)` enregistre par `TypeId` (échoue vite sur
doublons) → `.build()` produit un `InMemoryCommandBus` immuable backé par `Arc`. `MiddlewarePipeline::new(raw)`
le décore ensuite ; le **premier** `.layer()` est le plus externe.

**Dispatch (chemin chaud).** Un handler de transport enveloppe le payload en `Envelope::new(correlation_id,
payload)` et appelle `bus.dispatch(env)`. La chaîne décorée tourne (ex. Validation → Idempotency → Tracing →
Logging → `InMemoryCommandBus`) : un `HashMap::get` (O(1), sans verrou) + un `Box::new` pour franchir la
frontière d'effacement, puis `TypedHandlerBridge<H,C>` appelle `Arc<H>.handle(envelope)`. Clone du bus =
`Arc::clone`.

**Chaînage causal.** Dans un handler, `Envelope::new_caused_by(&incoming, payload)` hérite `correlation_id` +
métadonnées et fixe `causation_id`, pour qu'un flot multi-étapes garde une seule trace.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `validate-core` | amont | Separated Interface | supertrait `Validate` sur `Command` | la validabilité de chaque commande |
| `error` | amont | Conformist | `CqrsError: AppError`, `Error: AppError` du handler | la délégation d'erreur |
| chaque service | aval | Published Contract | bus command/query | tout le dispatch applicatif |
| `validation` | aval | Open-Host (étend) | `ValidationLayer: CommandLayer` | le middleware de validation d'entrée |
| `auth-context` | aval | Open-Host (étend) | `inject_into_envelope` (`cqrs-integration`) | la propagation d'identité dans les enveloppes |

> **Seam de stabilité :** les traits `Command`/`Query`/`*Handler`/`*Bus` et `Envelope<T>` sont une API
> publique ; la non-object-safety des traits de dispatch est un contrat que les appelants doivent respecter
> (tenir des types concrets).

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| span `cqrs.command.dispatch` / `cqrs.query.dispatch` | `tracing` (`TracingLayer`) | chaque dispatch (`otel.kind=INTERNAL`, `message.type/id`, `correlation.id`) | back-ends de trace distribuée |
| log start / complete-or-failed | `tracing` (`LoggingLayer`) | chaque dispatch (`elapsed_ms`, `error.code`) | dashboards latence + taux d'erreur |

La surface d'effet de bord est le `IdempotencyStore` qu'il écrit ; le store in-memory fourni est local au
processus et non borné.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Routage TypeId, pas de réflexion/vtable ; `dyn` uniquement dans des bridges scellés `pub(crate)` | [`README §Architecture`](../README.md) | Accepted |
| Pas de `async_trait` — RPIT natif de bout en bout ; `BoxFuture` uniquement dans les bridges effacés | [`README §Architecture`](../README.md) | Accepted |
| L'idempotence ne marque que sur `Ok` (les handlers échoués restent retentables) | [`README §Architecture`](../README.md) | Accepted |
| Séparation écriture/lecture imposée par le système de types | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — un bus CQRS ; le levier est le chemin chaud sans overhead et une forme de
  pipeline uniforme à travers chaque service.
- **Stabilité :** contrat stable — les traits et l'enveloppe sont stabilisés.
- **Volatilité :** faible — la croissance est de nouvelles couches fournies, écrites selon les mêmes invariants
  de moteur (pas de `async_trait`, aucune allocation sur le chemin commun).
- **Capacités différées :** un `IdempotencyStore` durable/TTL (Redis `SET NX EX`) — le seam existe ; le défaut
  est in-memory.
