---
i18n:
  source: ./DOMAIN.md
  source_sha256: 8d5ac66734950caf48ac4ffd737772d04aec7fee3e8a9daf97596381f6faa8b4
  translated_at: 2026-06-28
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`DOMAIN.md`](./DOMAIN.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, topics, variables
> d'environnement, noms de types, identifiants d'ADR) restent en anglais.

# `telemetry` — Contrat de Domaine & Fonctionnel

> Le bootstrap d'observabilité : logs + traces OTLP + métriques en un `init()`. Il répond à *« comment chaque binaire obtient-il une observabilité uniforme en un appel avec re-réglage live ? »*

> **Domain Card**
>
> | | |
> |---|---|
> | **Capacité partagée** | Bootstrap d'observabilité en un appel : logging structuré + tracing distribué OTLP + métriques Prometheus/OTLP, avec dials live sans verrou |
> | **Couche** | `platform` — l'unique bootstrap d'observabilité que chaque binaire appelle (via `service-runtime`) |
> | **Classe de sous-domaine** | **Supporting** — le substrat d'observabilité ; fort levier opérationnel (un schéma, re-réglage live) |
> | **Abstraction(s) primaire(s)** | `init` + `TelemetryGuard` + `TelemetryControl` (`telemetry`) |
> | **Empreinte** | IO/avec état — installe le subscriber global au processus, spawn les tâches OTLP/métriques ; nécessite un runtime Tokio |
> | **Posture en cas d'échec** | **tolérant aux pannes** — les buffers droppent à capacité plutôt que de backpressurer l'app ; les échecs d'export sont silencieux + retentés |
> | **Dépend de** | `tracing(-subscriber)`, `opentelemetry(_sdk)`, `arc-swap`, `prometheus`/`axum` (optionnel) |
> | **Consommé par** | `service-runtime` (appelle `init` dans `serve`) ; les crates de stockage émettent dans le subscriber installé |
> | **Journal des décisions** | aucun — justification dans [`README §Architecture`](../README.md) |

---

## 1. Capacité Technique & Non-Objectifs &nbsp;·&nbsp; CORE

**Capacité.** `telemetry` fait autorité dans la flotte pour **le pipeline d'observabilité** : il répond à
**« comment un binaire monte-t-il logs, traces, et métriques avec un schéma et un appel — et re-règle le filtre
de log et le sampling à chaud pendant un incident sans redémarrage ? »**

**Le problème difficile.** Trois pipelines indépendants (logging, tracing, métriques) doivent s'initialiser dans
le bon ordre, flush dans le bon ordre au shutdown, ne jamais bloquer le chemin chaud, et exposer des dials live
sans verrou pour la réponse aux incidents. `telemetry` câble les trois derrière un `init()` retournant un guard
à portée de durée de vie dont le drop flush spans → métriques → logs dans l'ordre, et un `TelemetryControl` qui
swap le filtre de log et le sampler via `ArcSwap`.

**Non-objectifs — ce que ce crate ne fait délibérément PAS :**
- ❌ Définir les métriques applicatives → les services obtiennent un meter du global OTel.
- ❌ Sonder la santé ou posséder la readiness → c'est `health` + `service-runtime`.
- ❌ Décider quand/où il est appelé → dans la flotte, `service-runtime` appelle `init`, pas les services directement.

---

## 2. Langage Omniprésent &nbsp;·&nbsp; CORE

| Terme | Sens dans ce crate | Symbole de code |
|---|---|---|
| Init | L'unique appel de bootstrap ; installe le subscriber global + les pipelines | `init`, `TelemetryConfig` |
| Guard | Le handle à portée de durée de vie dont le drop flush tout dans l'ordre | `TelemetryGuard` |
| Control | Les dials live clonables (filtre de log + sampling) | `TelemetryControl` |
| Sampling strategy | La policy de head-sampling, parent-based pour garder les traces entières | `SamplingStrategy::{AlwaysOn, AlwaysOff, TraceIdRatio}` |
| Metrics exporter | Pull Prometheus ou push OTLP | `MetricsExporterKind`, `PrometheusHandle` |

---

## 3. Modèle Public & Surface de Contrat &nbsp;·&nbsp; CORE

| Élément | Nature | Frontière de contrat / invariant gardée |
|---|---|---|
| `init(config)` | point d'entrée | Appeler **une fois**, dans un runtime Tokio, avant toute macro `tracing::` |
| `TelemetryGuard` | guard RAII | Lier à une var **nommée** ; drop = flush ordonné (spans → métriques → logs) ; les erreurs s'impriment, ne panic jamais |
| `TelemetryControl` | dials live | `set_log_filter`/`set_sampling` swap sans verrou, sans redémarrage ; le sampling parent-based garde les traces entières |
| `TelemetryConfig::from_env` | config | Lit `RUST_LOG`/`OTEL_*`/`LOG_FORMAT`/`METRICS_EXPORTER` |
| `PrometheusHandle` / `metrics_route` | surface de feature | `prometheus-exporter` (activé par défaut) ; texte `GET /metrics` |

---

## 4. Propriété & Frontières Architecturales &nbsp;·&nbsp; CORE

**Ce crate possède :**
- La construction du pipeline, le shutdown ordonné, et les dials de re-réglage live. Un schéma de log, une
  convention d'attributs de span, une convention de labels de métriques.

**Ce crate ne possède délibérément PAS / ne doit PAS lier :**

| Préoccupation | Vit dans | Pourquoi l'arête pointe ainsi |
|---|---|---|
| Les définitions de métriques applicatives | chaque service (via le meter global OTel) | Le crate fournit le pipeline, pas les métriques métier |
| Le parsing de la section de config `[telemetry]` | `infra-config` | Il expose un `TelemetrySink` ; `service-runtime` fait le pont (les deux crates ne doivent pas dépendre l'un de l'autre) |
| Santé/readiness | `health` + `service-runtime` | Préoccupation séparée |

**La liste « do-not-depend-on » :** jamais `infra-config` (le pont vit dans `service-runtime`), jamais un crate
de service. `prometheus`/`axum` sont optionnels (gatés par feature).

---

## 5. Invariants & Règles de Contrat &nbsp;·&nbsp; CORE

| # | Invariant | Appliqué à | En cas de violation |
|---|---|---|---|
| I1 | `init` est appelé exactement une fois (le slot global `tracing` est unique) | subscriber global | `TelemetryError::SubscriberInit` |
| I2 | `init` tourne dans un runtime Tokio, avant toute macro `tracing::` | OTel SDK | panic (les tâches spawn) / événements précoces droppés |
| I3 | Le guard est lié à une var nommée et survit au processus | RAII | flush précoce ; logs/spans disparaissent à la sortie |
| I4 | Le chemin chaud ne bloque jamais sur la télémétrie (drop-à-capacité, échecs d'export silencieux) | writers non-bloquants + export batch | perte de données préférée au backpressure |
| I5 | Le sampling est parent-based pour que les traces distribuées restent entières | `DynamicSampler` | traces cassées quand le volume est réduit |

---

## 6. Flot de Contrôle & Cycle de Vie &nbsp;·&nbsp; DEEP

**Init.** Construire un `Registry` avec `EnvFilter` (`RUST_LOG`|`LOG_FILTER`) + une couche de log non-bloquante
JSON/Pretty (`tracing_appender`) + une couche de trace OTLP (gRPC:4317 | HTTP:4318 → `BatchSpanProcessor` →
`TracerProvider` {Resource, Sampler}) → `try_init()` installe le subscriber global au processus. Un pipeline de
métriques (pull Prometheus ou push OTLP toutes les 60s) est construit indépendamment. Retourne
`TelemetryGuard { _log_guard, tracer_provider, metrics_pipeline }`.

**Dials live.** `TelemetryControl` tient un handle `tracing_subscriber::reload` (filtre de log) et un
`DynamicSampler` (`ShouldSample` sur `ArcSwap`) ; les deux swap sans verrou et sans redémarrage. `service-runtime`
enregistre ce control comme le sink de la section de config `[telemetry]`, pour qu'un push de ConfigMap re-règle
la flotte.

**Shutdown.** Le drop du guard flush dans l'ordre : `TracerProvider::shutdown()` (spans) →
`MetricsPipeline::shutdown()` → `WorkerGuard` (join le thread de log). Les erreurs s'impriment sur stderr, ne
panic jamais. Un second `init()` retourne `SubscriberInit` plutôt que d'écraser le premier.

---

## 7. Couplage de Crate (tranche du graphe de dépendances) &nbsp;·&nbsp; DEEP

| Crate voisin | Direction | Pattern | Mécanisme | Ce qui casse s'il change |
|---|---|---|---|---|
| `service-runtime` | aval | Published Contract | `init` + `TelemetryControl` | le boot d'observabilité + les dials live |
| `transport` | aval | Conformist | propagateur global + versions OTel épinglées | la propagation de trace (contexte compatible wire) |
| crates stockage/service | aval | Conformist | émettent dans le subscriber installé | l'apparition même de leurs logs/spans |
| `infra-config` | indirect | Separated Interface | `TelemetrySink` (ponté dans `service-runtime`) | le re-réglage live piloté par config |

> **Seam de stabilité :** `init`/`TelemetryGuard`/`TelemetryControl` sont une API publique ; les versions OTel
> épinglées sont un contrat de compatibilité wire avec `transport`.

---

## 8. Signaux Émis & Effets de Bord &nbsp;·&nbsp; DEEP

| Signal | Nature | Émis quand | Qui observe |
|---|---|---|---|
| gauges de process | métriques Prometheus | `prometheus-exporter` activé | `process_cpu_seconds_total`, `process_open_fds`, RSS, … |
| spans / métriques exportés | OTLP | batché hors du chemin chaud | le collector (Jaeger/Honeycomb/Datadog) |
| logs/spans/métriques de chaque service | relayés | dès qu'un service instrumente | c'est le substrat qu'ils chevauchent |

Effets de bord : installe le subscriber global, spawn les tâches Tokio batch-span + OTLP-reader, sert
optionnellement `/metrics`.

---

## 9. Décisions & Justification &nbsp;·&nbsp; DEEP

| Décision | Où consignée | Statut |
|---|---|---|
| Un `init()` pour les trois pipelines ; drop du guard = flush ordonné | [`README §Architecture`](../README.md) | Accepted |
| Dials live sans verrou (`TelemetryControl`) pour le filtre de log + sampling parent-based | [`README §Architecture`](../README.md) | Accepted |
| Tolérant aux pannes par conception (drop-à-capacité, retries d'export silencieux) | [`README §Architecture`](../README.md) | Accepted |
| Le pont vers `infra-config` vit dans `service-runtime` (éviter une dépendance circulaire) | [`service-runtime README`](../../service-runtime/README.md) | Accepted |

---

## 10. Classification & Évolution &nbsp;·&nbsp; DEEP

- **Classification :** Supporting — le substrat d'observabilité ; le levier est un schéma à l'échelle de la
  flotte + le re-réglage live.
- **Stabilité :** contrat stable.
- **Volatilité :** faible-moyenne — les options d'exporter/back-end évoluent ; la forme `init`/guard/control est stabilisée.
- **Capacités différées :** aucune structurelle ; de nouveaux exporters ou stratégies de sampling sont des enums additifs.
