---
i18n:
  source: ./DOMAIN.md
  source_sha256: 04ef012a3280e5878afb81cba74a5dabf21e4f1b782a1f62de217e7bfcdd6889
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `transport` — Contrat de Domaine & Fonctionnel

> La couche de communication partagée : gRPC + Kafka derrière une API, répondant à *« comment n'importe quel service parle-t-il à n'importe quel autre avec des traces de bout en bout et une livraison at-least-once, gratuitement ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Communication inter-services sur deux paradigmes (gRPC sync + Kafka async) avec propagation de trace W3C automatique et un runtime de consommateur obligatoire |
> | **Couche** | `platform` — l'unique couche wire que chaque service utilise |
> | **Classe de sous-domaine** | **Generic** — transport de commodité ; le levier est le tracing uniforme + le standard `run_consumer` |
> | **Abstraction(s) primaire(s)** | `GrpcClientBuilder`/`GrpcServerBuilder`, `KafkaProducerHandle`/`KafkaConsumerHandle`, `run_consumer` (`transport::{grpc, kafka}`) |
> | **Empreinte** | IO/avec état — possède les sockets, le client Kafka, et la boucle de consommateur |
> | **Posture en cas d'échec** | **fail-fast en sortie** (`CircuitOpen`/`Timeout`) + **Kafka at-least-once** (commit uniquement après une issue terminale) |
> | **Dépend de** | `tonic`/`tower`, `rdkafka`, `resilience`, `traffic`, `telemetry`, `error`, `opentelemetry` |
> | **Consommé par** | chaque service (clients/serveurs gRPC, producteurs/consommateurs Kafka) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `transport` fait autorité dans la flotte pour **le wire** : il répond à
**« comment un service appelle-t-il un autre (gRPC) ou publie/consomme un événement (Kafka) de sorte que les
traces se propagent de bout en bout et que les sémantiques de livraison soient uniformes ? »** — sans
boilerplate par service.

**Le problème difficile.** Deux transports, une seule histoire de trace. Les deux doivent auto-propager le
TraceContext W3C (`traceparent`/`tracestate`), câbler les couches de résilience (sortie) et de traffic (entrée),
et donner à Kafka un *unique* runtime de consommateur at-least-once correct pour que le comportement
retry/DLQ/offset soit identique à l'échelle de la flotte plutôt que réinventé (et subtilement cassé) par service.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Posséder les types de domaine, schémas, ou la logique des handlers → il ne possède que le wire + la plomberie de trace.
- ❌ Retenter au niveau du channel gRPC → les bodies HTTP/2 sont des streams (coût de buffering) ; le retry va à la couche app.
- ❌ Définir le *mécanisme* résilience/traffic → ce sont `resilience`/`traffic` ; ce crate les câble.
- ❌ Initialiser la télémétrie → `telemetry::init()` est un prérequis dur (enregistre le propagateur).

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Resilient channel | Un channel gRPC enveloppé de trace + circuit-breaker + timeout | `ResilientChannel`, `GrpcClientBuilder::build_resilient` |
| Traced server | Un serveur gRPC avec couches inbound-trace + ingress-traffic préinstallées | `GrpcServerBuilder`, `TracedGrpcServer` |
| Event envelope | Le porteur de publication Kafka typé | `EventEnvelope<T>`, `PublishablePayload` |
| Consumed message | Un message Kafka entrant décodé (erreur de décode = `payload: Err`, pas un abort du stream) | `ConsumedMessage<T>`, `ConsumablePayload` |
| Consumer runtime | La machine à états par message obligatoire (retry/DLQ/commit) | `run_consumer`, `ProcessOutcome`, `ClassifyError` |
| Propagation | Inject/extract du contexte de trace sur les deux transports | `inject_context`, `extract_context`, `set_parent` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `TransportError` | enveloppe d'erreur | Aplatit gRPC/Kafka/Codec + `CircuitOpen`/`Timeout`/`MaxRetriesExhausted` |
| `ResilientChannel` | alias de type | `BoxCloneService<…, TransportError>` ; `Clone` bon marché ; lit CB/timeout d'un `ArcSwap` `ResilienceProfile` |
| `KafkaProducerHandle` | handle | Backé par `Arc`, `Clone` ; `publish` injecte le contexte de trace dans les headers |
| `KafkaConsumerHandle` | handle | `stream` (erreur décode ≠ abort) + `commit` (offset+1, commit manuel par défaut) |
| `run_consumer` | runtime | Possède la machine à états retry/DLQ/commit — **obligatoire** pour chaque consommateur |
| `ProcessOutcome` | enum | `Done`/`Retry`/`Reject` pilotent la décision terminale-ou-redélivrée du runner |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- Le wire (clients/serveurs gRPC + Kafka), la plomberie de trace, le *câblage* des couches résilience/traffic,
  et le runtime de consommateur. Les sémantiques de livraison (le standard `run_consumer`) sont possédées ici.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Le mécanisme de résilience (CB/retry/timeout) | `resilience` | Ce crate le *câble* ; il ne le définit pas |
| Le limiteur de traffic | `traffic` | Idem — `TrafficLayer` est câblé mais inerte tant qu'aucun registry n'est fourni |
| Le pipeline télémétrie + le propagateur global | `telemetry` | `transport` exige `init()` d'abord ; il ne possède pas le pipeline |
| Les schémas d'événements de domaine / la logique des handlers | crates de service | Le transport porte des payloads opaques + le contexte de trace |

**La liste « do-not-depend-on » :** jamais un crate de service/domaine. Les versions OTel sont épinglées sur
celles de `telemetry` pour un contexte compatible au niveau wire.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | `telemetry::init()` tourne avant tout appel transport (sinon inject/extract sont des no-ops) | prérequis runtime | spans déconnectés / `traceparent` manquant |
| I2 | Un offset Kafka commit uniquement après une issue **terminale** (succès ou dead-letter réussi) | `run_consumer` | un message poison est évacué sans perte |
| I3 | Une erreur broker/stream ou un échec de publication DLQ retourne `Err` **sans** committer | `run_consumer` | reprise au dernier offset committé, aucune perte |
| I4 | Un échec de décode dead-letter immédiatement (n'abort pas le stream) | `stream` + `run_consumer` | poison isolé dans la DLQ |
| I5 | Aucun `RetryLayer` au niveau du channel | composition de la stack client | (le retry relève de la couche app) |
| I6 | L'idempotence est la responsabilité du consommateur (at-least-once ⇒ vraie redélivrance) | convention de contrat | effets de bord dupliqués |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Stack client gRPC.** `TimeoutLayer → CircuitBreakerLayer → OutboundTraceLayer → tonic Channel`
(`ResilientChannel`). La couche outbound injecte `traceparent`/`tracestate` dans les headers HTTP/2 ; CB et
timeout lisent des valeurs hot-reloadables depuis l'`ArcSwap` du `ResilienceProfile` d'origine.

**Stack serveur gRPC.** `InboundTraceLayer` (externe — trace même les requêtes throttlées) `→ TrafficLayer`
(limite en entrée, inerte tant que `service-runtime` ne fournit pas un `TrafficRegistry` ; le mode shadow charge
les cellules sans rejeter) `→ handler`.

**Consommateur Kafka (`run_consumer`).** Par message : décode (`payload: Err` ⇒ dead-letter + commit) ; sinon
lance `process` → `ProcessOutcome` : `Done` ⇒ commit ; `Retry` ⇒ backoff+jitter en place jusqu'à
`max_attempts`, puis dead-letter + commit ; `Reject` ⇒ dead-letter + commit. Une erreur broker ou un **échec de
publication DLQ** ⇒ retourne `Err` sans committer, pour que l'appelant rebuild et reprenne au dernier offset
committé. Les enregistrements DLQ portent `x-dlq-origin-*` + le contexte de trace.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `resilience` | amont | Conformist | `*Layer` + `ResilienceProfile` | le câblage de résilience en sortie |
| `traffic` | amont | Conformist | `check` / `TrafficDecision` → `RESOURCE_EXHAUSTED` | le limiting en entrée |
| `telemetry` | amont | Conformist | propagateur global + versions OTel épinglées | la propagation de trace |
| `error` | amont | Conformist | `grpc_severity`, mapping d'erreur | la sévérité d'erreur gRPC |
| chaque service | aval | Published Contract | builders client/serveur + `run_consumer` | toutes les communications inter-services |
| `traffic-redis` | aval-de-nous (injecté) | Separated Interface | `with_traffic_backend(Arc<dyn QuotaBackend>)` | le rate limiting distribué |

> **Seam de stabilité :** `run_consumer` + sa table de livraison est un **standard de flotte obligatoire** ;
> `TransportError`, les builders, et les handles Kafka sont une API publique.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| span `grpc.server` | `tracing`/OTel | chaque RPC entrant (`rpc.system=grpc`, `rpc.method`) | back-ends de trace |
| `traceparent`/`tracestate` injectés | header wire | chaque appel client gRPC + publication Kafka | l'extract du récepteur |
| enregistrement DLQ | effet de bord Kafka | une issue terminale `Retry`-épuisé/`Reject`/échec de décode | consommateurs DLQ / ops |
| `infra_traffic_throttled_total{status}` | métrique (via câblage traffic) | une décision `Throttle` (shadow ou enforce) | dashboards de rate-limit |

Effets de bord : ouvre des sockets, publie/consomme Kafka, écrit des enregistrements DLQ, commit des offsets.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Les deux transports auto-propagent le TraceContext W3C | [`README §Architecture`](../README.md) | Accepted |
| Aucun `RetryLayer` au niveau transport (buffering de body HTTP/2) — retry à la couche app | [`README §Architecture`](../README.md) | Accepted |
| `run_consumer` est le runtime de consommateur obligatoire ; commit uniquement après une issue terminale | [`README §Consumer runtime standard`](../README.md) | Accepted |
| Traffic câblé-mais-inerte tant que non configuré ; mode shadow avant enforce | [`README §Architecture`](../README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Generic — transport de commodité ; la différenciation est le tracing uniforme + un
  unique runtime de consommateur at-least-once correct.
- **Stabilité :** contrat stable — `run_consumer` est un standard de flotte ; les builders/handles sont stabilisés.
- **Volatilité :** faible-moyenne — la croissance est dans l'observabilité (les instruments de meter Prometheus
  sont un TODO) et les boutons de config, pas la forme.
- **Capacités différées :** instruments de meter Prometheus pour les métriques niveau transport ; déploiement
  TLS/mTLS plus riche.
