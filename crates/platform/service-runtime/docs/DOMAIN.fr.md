---
i18n:
  source: ./DOMAIN.md
  source_sha256: 01aab4383d39a7a33a052b5a2fe577957a3e3ce9a69a5416e158964d079fd8c9
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `service-runtime` — Contrat de Domaine & Fonctionnel

> Le bootstrap unifié de la flotte : implémentez un trait, obtenez un service déployable. Il répond à *« quelle est l'unique séquence de boot que chaque service exécute, et que possède encore un service ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | L'unique séquence de boot que chaque service exécute : telemetry → config + hot-reload → compose → serve (trace + traffic + health) → drain |
> | **Couche** | `platform` — la composition root partagée par chaque binaire `*-server` |
> | **Classe de sous-domaine** | **Supporting** — l'épine dorsale opérationnelle ; un seul endroit pour faire évoluer les préoccupations process à l'échelle de la flotte |
> | **Abstraction(s) primaire(s)** | Le trait `Service` + `serve::<S>(addr)` (`service_runtime`) |
> | **Empreinte** | IO/avec état — bind les sockets, spawn les boucles watcher + readiness + prune, possède le shutdown |
> | **Posture en cas d'échec** | **fail-closed au boot** (une mauvaise config ne sert jamais) + **santé dynamique** (`NOT_SERVING` jusqu'à ce que les probes passent) |
> | **Dépend de** | `tonic`, `telemetry`, `infra-config`, `traffic`, `health`, `error`, `transport` |
> | **Consommé par** | chaque binaire `crates/apps/<svc>-server` (via `serve::<S>(addr)`) |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `service-runtime` fait autorité dans la flotte pour **la séquence de boot** : il répond à
**« comment chaque service démarre, observe, configure, limite le débit, rapporte sa santé, et drain de façon
identique — pour qu'un binaire de service soit un one-liner ? »** La séparation est délibérée : le runtime
possède les préoccupations process ; le service ne possède que son câblage de domaine, ses services gRPC
concrets, et ses probes backend.

**Le problème difficile.** Les préoccupations process (observabilité, IO config + hot-reload, rate-limiting en
entrée, bind de socket, shutdown gracieux, la boucle de readiness qui pilote la santé gRPC depuis la liveness
backend) sont identiques à travers 17 services et faciles à rater subtilement. Les centraliser derrière un trait
— avec un seam `RoutesBuilder` type-erased pour que la stack de couches Tower ne fuite jamais dans la signature
du service — fait qu'un nouveau service est `~10` lignes et qu'un changement à l'échelle de la flotte est une
seule édition ici.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Posséder le câblage de domaine (repos, caches, bus, workers) → le `build` du service.
- ❌ Définir les *mécanismes* telemetry/config/traffic/health → ce sont `telemetry`/`infra-config`/`traffic`/`health`.
- ❌ Exposer les types de couches Tower aux services → le seam `register` les masque.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Service | Le trait qu'un service déployable implémente (la seule surface) | `Service` |
| Composition root | L'étape pure de construction du graphe du service | `Service::build` |
| Register | Brancher les services gRPC concrets du service sur un builder type-erased | `Service::register`, `RoutesBuilder` |
| Readiness loop | La boucle de fond mappant probes → statut de santé gRPC | `spawn_readiness` |
| Traffic prune loop | La boucle de fond bornant la mémoire du rate-limiter | `spawn_traffic_prune` |
| Telemetry control sink | Le pont appliquant la config `[telemetry]` au pipeline vivant | `TelemetryControlSink` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `Service` | trait (seam) | Consts `NAME`/`VERSION`/`GRPC_SERVICE_NAME` + `build`/`health_probes`/`register` |
| `serve::<S>(addr)` | point d'entrée | Tout le boot+serve+drain de production ; un binaire n'est que cet appel |
| `GRPC_SERVICE_NAME` | const de contrat | **Doit** égaler le `NamedService::NAME` du serveur concret (la clé de santé) |
| ré-exports | ergonomie | `HealthProbe`/`FnProbe` (de `health`), `InfraRegistry` (de `infra-config`) |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**

| Préoccupation | Propriétaire |
|---|---|
| Init télémétrie, OTLP, dials log/sampling | **runtime** |
| Chargement config + watcher hot-reload + pont telemetry sink | **runtime** |
| Couches trace + rate-limit en entrée, boucle de prune | **runtime** |
| Santé gRPC, boucle de readiness, shutdown gracieux | **runtime** |

**Le service possède** (pas ce crate) : le câblage de domaine (`build`), les services gRPC concrets + reflection
(`register`), les probes backend (`health_probes`).

**La liste « do-not-depend-on » :** il compose les crates platform/foundation mais ne possède aucun de leurs
mécanismes ; il ne doit pas tirer un crate de service/domaine. Le pont `TelemetryControlSink` vit ici précisément
parce qu'il a besoin à la fois de `infra-config` et `telemetry`, qui ne doivent pas dépendre l'un de l'autre.

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | Le boot est fail-closed : une config malformée empêche le pod de jamais servir | `serve` (chargement config) | le pod ne devient jamais ready |
| I2 | `GRPC_SERVICE_NAME` doit égaler le `NamedService::NAME` concret | convention de contrat | le client voit `NOT_SERVING` à jamais |
| I3 | Avec des probes, un service est `NOT_SERVING` jusqu'à ce que toutes passent ; tout échec le rétrograde | `spawn_readiness` | la readiness reflète la joignabilité backend réelle |
| I4 | Le guard du watcher de config survit au processus | `serve` garde `_watcher` en scope | la config gèle à la valeur de boot |
| I5 | Les types de couches Tower n'atteignent jamais `register` | seam `RoutesBuilder` type-erased | types de couches fuités dans les signatures de service |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**`serve::<S>(addr)` — l'unique séquence de boot.**

1. `telemetry::init` (logs + traces OTLP + métriques) ; le guard est gardé (le drop flush spans/logs). *(boot)*
2. `load_from_path` + `InfraRegistry::from_config` — **fail-closed** ; un mauvais document abort le boot. *(boot)*
3. Enregistrer le `TelemetryControlSink` pour que les dials `[telemetry]` s'appliquent immédiatement et à chaque changement ultérieur. *(boot)*
4. `spawn_watcher` (gardé vivant) — hot-reload de resilience/cache/traffic/telemetry. *(fond)*
5. `S::build(infra)` — la composition root du service. *(boot)*
6. Construire le serveur gRPC : `InboundTraceLayer` (externe) + `TrafficLayer` (interne, seulement si `[traffic]`
   présent) ; ajouter le service de santé + `S::register(routes)`. *(boot)*
7. `spawn_readiness` (probes → santé gRPC, écritures uniquement sur transition) + `spawn_traffic_prune` (borne la
   mémoire du limiteur). *(fond)*
8. `serve_with_shutdown` — servir jusqu'au SIGINT, puis drain les requêtes en vol. *(durée de vie → shutdown)*

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `telemetry` | amont | Conformist | `init` + `TelemetryControl` | le boot d'observabilité + les dials live |
| `infra-config` | amont | Conformist | `load_from_path`/`spawn_watcher`/`InfraRegistry` | le boot config + hot-reload |
| `transport` | amont | Conformist | `GrpcServerBuilder` (+ traffic) | la stack serveur gRPC |
| `health` | amont | Conformist | `HealthProbe` (ré-exporté) | la boucle de readiness |
| chaque binaire `*-server` | aval | Published Contract | `impl Service` + `serve::<S>` | le boot de toute la flotte |

> **Seam de stabilité :** le trait `Service` (surtout `GRPC_SERVICE_NAME` ↔ `NamedService::NAME`) est l'unique
> surface à laquelle chaque service se lie ; le changer touche les 17.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| `gRPC server listening` / `shutdown complete` | `tracing` INFO | boot / drain | ops |
| `gRPC health status changed` | `tracing` INFO | une transition de readiness | les readiness probes K8s |
| `traffic registry pruned` | `tracing` DEBUG | chaque tick de prune | monitoring mémoire du limiteur |

Effets de bord : bind le socket d'écoute, spawn les tâches watcher/readiness/prune, installe le handler SIGINT.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Un trait `Service` possède la séquence de boot ; un binaire est un one-liner | [`README §Architecture`](../README.md) | Accepted |
| Le seam `RoutesBuilder` type-erased garde les couches Tower hors des signatures de service | [`README §Architecture`](../README.md) | Accepted |
| Santé gRPC dynamique pilotée par les probes backend (pas épinglée `SERVING` au boot) | [`README §Architecture`](../README.md) | Accepted |
| Boot config fail-closed + hot-reload à écrivain unique | [`infra-config README`](../../../foundation/infra-config/README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Supporting — l'épine dorsale opérationnelle ; le levier est l'uniformité des
  préoccupations process à l'échelle de la flotte.
- **Stabilité :** contrat stable — le trait `Service` est stabilisé à travers 17 services.
- **Volatilité :** faible — les nouvelles préoccupations process (une nouvelle boucle de fond, une nouvelle
  couche) sont ajoutées ici une fois.
- **Capacités différées :** câblage SIGTERM→drain au-delà de SIGINT ; des hooks de drain/health plus riches pour
  les services edge avec état (notés dans le travail realtime).
