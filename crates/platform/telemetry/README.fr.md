---
i18n:
  source: ./README.md
  source_sha256: fc26e710a97d54ae5b6889f29951a7f7a35cd9ff069df09436dea3380227b4a9
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `telemetry` — Bootstrap d'observabilité unifié : logs + traces OTLP + métriques en un `init()`

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `platform` — l'unique bootstrap d'observabilité que chaque binaire appelle |
> | **Package** | `telemetry` (dir : `crates/platform/telemetry`) |
> | **Consommé par** | `service-runtime` (appelle `init` dans `serve`) ; les crates de stockage émettent dans le subscriber installé |
> | **Dépend de** | `tracing(-subscriber)`, `opentelemetry(_sdk)`, `arc-swap`, `prometheus`/`axum` (optionnels) |
> | **Stabilité** | contrat stable |
> | **Feature flags** | `prometheus-exporter` (défaut **on**) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`telemetry` est l'unique crate d'observabilité de référence : il câble logging structuré, tracing
distribué OTLP et métriques Prometheus/OTLP en un seul appel `init()`, renvoyant un `TelemetryGuard` à
durée de vie scopée qui possède tous les handles d'arrêt de pipeline. Chaque binaire l'appelle avant de
servir — un schéma de log, un jeu d'attributs de span, une convention de labels de métriques, sans
câblage par service.

**Frontière architecturale** — il possède la construction du pipeline + l'arrêt + les dials de retuning
live. Il ne définit **pas** les métriques applicatives (les services obtiennent un meter du global OTel)
et ne fait pas de polling de santé. Dans la flotte, les services n'appellent pas `init` directement —
`service-runtime` le fait.

**Objectifs fondamentaux :** un seul appel / zéro dérive ; arrêt gracieux (le drop du guard flush spans →
métriques → logs dans l'ordre) ; défauts safe-hyperscale (IO de log non bloquant, export de spans en
batch async, head-sampling à 10% — aucun ne bloque le chemin chaud).

---

## 📐 Architecture & décisions clés

```
init(config):
  Registry .with(EnvFilter: RUST_LOG|LOG_FILTER)
           .with(LogLayer: tracing_appender non-blocking → JSON | Pretty)
           .with(TraceLayer: OTLP gRPC:4317 | HTTP:4318 → BatchSpanProcessor → TracerProvider{Resource, Sampler})
           .try_init()                                   ← process-global subscriber
  + Metrics pipeline (independent): Prometheus (pull /metrics) | OTLP (push every 60s)
  → TelemetryGuard { _log_guard, tracer_provider, metrics_pipeline }   (must live until process exit)
```

- **Drop du guard = flush ordonné** — au drop : `TracerProvider::shutdown()` (flush spans) →
  `MetricsPipeline::shutdown()` → `WorkerGuard` (join du thread de log). Les erreurs vont sur stderr,
  jamais de panic.
- **Dials live via `TelemetryControl`** — le filtre de log (un handle `tracing_subscriber::reload`) et le
  sampling (un `DynamicSampler` = `ShouldSample` sur `ArcSwap`) swappent tous deux **lock-free, sans
  redémarrage**. Le sampling est parent-based, donc les traces distribuées restent entières quand on
  baisse le volume en incident. `service-runtime` enregistre ce control comme sink de la section
  `[telemetry]`.
- **Tolérant aux pannes par conception** — les buffers de span/log **droppent** à saturation plutôt que
  de faire de la backpressure sur l'app ; les échecs d'export OTLP sont silencieux (réessayés au prochain
  batch/période). Un second `init()` renvoie `SubscriberInit` au lieu d'écraser le premier pipeline.

---

## 🔌 API publique & contrat

```rust
pub fn init(config: TelemetryConfig) -> Result<TelemetryGuard, TelemetryError>;   // call ONCE, before any tracing:: macro

pub struct TelemetryConfig { pub service_name: String, pub service_version: String, pub log: LogConfig, pub trace: TraceConfig, pub metrics: MetricsConfig }
impl TelemetryConfig { pub fn from_env(service_name: impl Into<String>, service_version: impl Into<String>) -> Self; }

pub enum LogFormat { Json, Pretty }
pub enum OtlpProtocol { Grpc, HttpProtobuf }
pub enum SamplingStrategy { AlwaysOn, AlwaysOff, TraceIdRatio(f64) }   // ratio ∈ [0,1], default 0.1
pub enum MetricsExporterKind { Prometheus, Otlp { endpoint: String } }

pub struct TelemetryGuard;
impl TelemetryGuard {
    pub fn prometheus_handle(&self) -> Option<Arc<PrometheusHandle>>;  // None for OTLP / feature off
    pub fn control(&self) -> TelemetryControl;                         // cloneable live dials
}
impl Drop for TelemetryGuard { /* spans → metrics → logs */ }

impl TelemetryControl { pub fn set_log_filter(&self, &str) -> Result<(),_>; pub fn set_sampling(&self, SamplingStrategy) -> Result<(),_>; }

// feature = "prometheus-exporter":
impl PrometheusHandle { pub fn render(&self) -> String; }             // text/plain; version=0.0.4
pub fn metrics_route(handle: Arc<PrometheusHandle>) -> impl Fn() -> /* Axum handler */ + Clone;

pub enum TelemetryError { OtlpExporter(String), Prometheus(String), SubscriberInit(String), InvalidSamplingRatio(f64) }
```

> **Contrat :** appeler `init` exactement une fois, depuis un contexte Tokio (le batch processor + le
> reader OTLP lancent des tâches Tokio — l'appeler hors d'un runtime panique dans le SDK OTel), avant
> toute macro `tracing::`. Lier le guard à une variable **nommée** (`let _guard = …`) — un `_` nu le
> droppe immédiatement et flush avant que rien ne soit enregistré.

---

## 📦 Intégration

```toml
[dependencies]
telemetry = { workspace = true }                          # Prometheus + Axum route helper by default
# telemetry = { workspace = true, default-features = false }  # pure OTLP push, drops axum+prometheus
```

```rust
let _guard = telemetry::init(TelemetryConfig::from_env("post-command-server", env!("CARGO_PKG_VERSION")))
    .expect("telemetry init failed");                     // BEFORE serving; keep _guard alive to flush on exit
let prom = _guard.prometheus_handle().unwrap();
let router = Router::new().route("/metrics", get(telemetry::metrics::exporter::metrics_route(prom)));
```

Retuning live (la flotte utilise `service-runtime` + la config `[telemetry]` ; API directe montrée) :
`_guard.control().set_log_filter("info,chat=debug")`, `.set_sampling(SamplingStrategy::TraceIdRatio(0.01))`.

---

## ⚙️ Configuration & feature flags

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | `tracing_subscriber` directives; précédence sur `LOG_FILTER` |
| `LOG_FILTER` | `info` | Fallback filter when `RUST_LOG` absent |
| `LOG_FORMAT` | `json` | `json` (prod) or `pretty` (dev) |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4317` | OTLP gRPC collector (traces + OTLP metrics) |
| `OTEL_EXPORTER_OTLP_HEADERS` | — | Auth headers `k=v,k2=v2` (Honeycomb/Datadog) |
| `OTEL_TRACES_SAMPLER_ARG` | `0.1` | Head-sampling ratio `[0,1]` |
| `METRICS_EXPORTER` | `prometheus` | `prometheus` (pull) or `otlp` (push) |
| `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT` | `http://localhost:4317` | OTLP endpoint when `METRICS_EXPORTER=otlp` |

**Feature flags :** `prometheus-exporter` (défaut on) — ajoute `opentelemetry-prometheus`, `prometheus`
(avec métriques process), `axum` ; expose `PrometheusHandle` + `metrics_route`. `default-features = false`
⇒ pas de déps Prometheus/Axum. **Le runtime Tokio est obligatoire.**

---

## 🔭 Observabilité

Le mode Prometheus auto-enregistre les gauges process : `process_cpu_seconds_total`, `process_open_fds`,
`process_max_fds`, `process_virtual_memory_bytes`, `process_resident_memory_bytes`,
`process_start_time_seconds`. Instrumenter les métriques applicatives via le global OTel :
`global::meter("svc").u64_counter("grpc.server.requests.total").build()`.

Alertes suggérées : erreurs d'export OTLP (logs collector) ⇒ critique ; `process_open_fds/max_fds > 0.85`
⇒ warn ; RSS monotone sur 10m ⇒ warn ; trou de log (worker en retard) ⇒ warn. Surcoût chemin chaud ~0 si
filtré ; création de span O(1), export en batch hors chemin ; inc de compteur = un atomic.

---

## 🧪 Tests

```bash
cargo test   -p telemetry
cargo test   -p telemetry --all-features
cargo test   -p telemetry --no-default-features      # verify the feature gate compiles
cargo clippy -p telemetry --all-targets
# local collector: docker run --rm -p4317:4317 -p16686:16686 jaegertracing/all-in-one + OTEL_TRACES_SAMPLER_ARG=1.0
```

Fichiers clés pour les contributeurs : `src/init.rs` (ordre des couches — à lire en premier),
`src/guard.rs` (séquence de drop), `src/trace/layer.rs`, `src/metrics/layer.rs`, `src/trace/exporter.rs`.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `TelemetryError::SubscriberInit` — « subscriber already initialised ».**
`init()` a été appelé deux fois (le slot global `tracing` est unique). L'appeler une fois en tête de
`main()` avant que toute bibliothèque n'installe son propre subscriber. En test, protéger avec un
`OnceLock<TelemetryGuard>`.

**2. Pas de spans dans le collector / « connection refused » à l'export.**
Le endpoint par défaut `http://localhost:4317` est injoignable en pod sans sidecar collector, et les
échecs d'export sont **silencieux** (spans droppés). Pointer `OTEL_EXPORTER_OTLP_ENDPOINT` sur le
collector du cluster ; mettre `OTEL_TRACES_SAMPLER_ARG=1.0` + `LOG_FORMAT=pretty` pour confirmer que les
spans sont créés avant d'accuser l'exporter ; définir `OTEL_EXPORTER_OTLP_HEADERS` pour l'auth SaaS.

**3. Tous les spans échantillonnés en prod / pic de coût.**
`OTEL_TRACES_SAMPLER_ARG=1.0` a fui d'une config dev (le défaut est `0.1`). Vérifier l'env déployé, mettre
`0.01`–`0.1` ; `0.0` désactive le tracing sans changer le binaire.

**4. Le pipeline n'a rien flush / les logs ont disparu à la sortie.**
Le guard a été lié à `_` (droppe immédiatement). Utiliser `let _guard = telemetry::init(...)?` et le garder
en scope jusqu'à la fin de `main()`.
