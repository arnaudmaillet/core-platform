---
i18n:
  source: ./README.md
  source_sha256: 0902a4fb299b16dca3422335d6de6b2315906520c2afb6f95e0ad08edee2e3af
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `resilience` — Middleware Tower contre les défaillances en cascade (circuit breaker · retry · timeout)

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — mécanisme middleware Tower pur (tolérance aux pannes en sortie) |
> | **Package** | `resilience` (dir : `crates/foundation/resilience`) |
> | **Consommé par** | `transport` (clients gRPC/Kafka), le bus `cqrs` ; config via `infra-config` |
> | **Dépend de** | `tower`, `arc-swap`, `tokio`, `thiserror`, `rand`, `serde` (optionnel), `error` |
> | **Stabilité** | contrat stable (production-ready, sans stubs) |
> | **Feature flags** | `serde` (off par défaut — ajoute les types filaires) |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`resilience` fournit des couches middleware Tower de qualité production protégeant les microservices des
défaillances en cascade à la frontière de transport **sortant** : un **circuit breaker** (échec rapide
quand un aval est mal en point), un **retry** (échecs transitoires avec backoff exponentiel + jitter), et
un **timeout** (deadline absolue par requête). Il se place entre les clients `transport` et le bus
`cqrs` — chaque appel sortant passe par ces couches, donc la politique de résilience est à l'échelle de
la flotte sans toucher à la logique métier. C'est le pendant en sortie de [`traffic`](../traffic)
(entrée).

**Frontière architecturale** — une **bibliothèque pure** : pas d'IO fichier, pas de `notify`, pas d'env,
pas de tâches lancées. Le crate reste pur ; ses **profils** nommés tiennent derrière des handles
`Arc<ArcSwap<_>>` pour un hot-reload lock-free, tandis que parsing/validation/bindings/surveillance de
fichier vivent dans [`infra-config`](../infra-config).

---

## 📐 Architecture & décisions clés

```
Caller (CQRS bus / gRPC handler)
   ▼  TimeoutLayer        → ResilienceError::Timeout         (total request budget; outermost)
   ▼  CircuitBreakerLayer → ResilienceError::CircuitOpen     (counts ALL attempts incl. retries)
   ▼  RetryLayer          → ResilienceError::MaxRetriesExhausted (backoff between attempts)
   ▼  Inner service (tonic client / Kafka producer / HTTP)
```

Machine à états du circuit breaker : `Closed → Open` après `failure_threshold` échecs consécutifs ; après
`open_duration`, `Open → HalfOpen` ; `HalfOpen → Closed` après `success_threshold` succès, ou retour à
`Open` sur un échec de sonde. `half_open_max_calls` plafonne les sondes concurrentes.

- **L'ordre des couches est porteur** — Timeout *enveloppe* CircuitBreaker *enveloppe* Retry. Le circuit
  doit être **à l'extérieur** du retry pour qu'il compte les retries comme tentatives et trip à temps ;
  inversez-les et le circuit ne voit que le premier appel.
- **Config échantillonnée une fois par `call`** — chaque opération `ArcSwap::load`e un seul snapshot pour
  raisonner sur des valeurs cohérentes ; les swaps de config sont lock-free et **ne réinitialisent jamais
  l'état live** (compteurs, timers, état du circuit).
- **Défaut `JitterKind::Full`** — distribue les délais de retry sur `[0, cap]`, donc une flotte qui
  réessaie le même aval après une panne ne pique pas en lockstep (atténuation du thundering herd).
- **Les types filaires `serde` font le pont sur la frontière générique** — `RetryConfig<B>` est générique
  pour un dispatch sans coût et ne peut être désérialisé ; les types non-génériques `…Spec` se
  désérialisent puis `resolve()` vers les types runtime monomorphisés.

---

## 🔌 API publique & contrat

```rust
pub enum ResilienceError<E> {       // thiserror::Error
    CircuitOpen,                    // downstream assumed down; request NOT forwarded
    Timeout(Duration),
    MaxRetriesExhausted(u32),
    Inner(E),                       // the ONLY variant carrying downstream error state
}

pub trait BackoffStrategy: Send + Sync + Clone + 'static { fn next_delay(&self, attempt: u32) -> Duration; } // attempt 1-indexed
pub struct ExponentialBackoff { pub base_ms: u64, pub max_ms: u64, pub jitter: JitterKind } // default 50ms / 10_000ms / Full
pub enum JitterKind { None, Full, Equal }   // exponent clamped to 30 to avoid u64 overflow

pub trait RetryPolicy<E>: Send + Sync + Clone + 'static { fn should_retry(&self, error: &E, attempt: u32) -> bool; }
// DefaultRetryPolicy (delegates to AppError::is_retryable) · AlwaysRetryPolicy · NeverRetryPolicy

// Layers — new(config) seeds a fresh ArcSwap; from_handle(...) shares one; handle() hands it back for control-plane store()
CircuitBreakerLayer::new(CircuitBreakerConfig) | ::from_handle(Arc<ArcSwap<_>>) | .handle()
RetryLayer::new(RetryConfig<B>, policy: P)
TimeoutLayer::new(TimeoutConfig) | ::from_handle(Arc<ArcSwap<_>>) | .handle()

// ResilienceProfile: bundles one timeout + CB + retry as a named class-of-service; timeout/CB behind shared ArcSwap.
impl ResilienceProfile { fn timeout_layer(&self); fn circuit_breaker_layer(&self); fn apply(&self, ResilienceProfileSpec) -> RetryConfig<ExponentialBackoff>; }
```

Structs de config (défauts) : `CircuitBreakerConfig { failure_threshold: 5, success_threshold: 2,
open_duration: 30s, half_open_max_calls: 1 }`, `RetryConfig { max_attempts: 3, backoff }`,
`TimeoutConfig { duration }`.

> **Contrat :** `Inner(E)` est la seule variante portant l'état aval ; le reste est émis par le
> middleware. `CircuitBreakerService`/`TimeoutService` sont `Clone` (les clones partagent le même état
> `Arc`) — requis par tonic, qui clone le service par RPC. `RetryService` exige `S: Clone` **et**
> `Req: Clone` (il ré-émet la requête par tentative). Avec `serde` on, les champs `Duration` sérialisent
> en entiers ms plats (`open_duration` ⇄ `open_duration_ms`).

---

## 📦 Intégration

```toml
[dependencies]
resilience = { workspace = true }   # add features = ["serde"] to parse config
```

```rust
use tower::ServiceBuilder;
use resilience::{circuit_breaker::*, retry::*, timeout::*};

// Order matters: Timeout (outer) → CircuitBreaker → Retry → inner.
let resilient = ServiceBuilder::new()
    .layer(TimeoutLayer::new(TimeoutConfig::from_secs(5)))
    .layer(CircuitBreakerLayer::new(CircuitBreakerConfig::new()
        .failure_threshold(5).open_duration(Duration::from_secs(30))
        .success_threshold(2).half_open_max_calls(1)))
    .layer(RetryLayer::new(RetryConfig::default_exponential(), DefaultRetryPolicy))
    .service(inner_grpc_client);   // inner must be cheaply Clone (Arc-backed or tower::Buffer)
```

Charger/valider les profils et résoudre les bindings (`"post-command" → "critical"`) vit dans
[`infra-config`](../infra-config).

---

## ⚙️ Configuration & feature flags

Bibliothèque pure — **pas de variables d'environnement, pas de processus**. La config est passée
programmatiquement (usage statique) ou sourcée en externe et appliquée via les handles
`ResilienceProfile` (hot-reload via `infra-config`).

**Feature flags :**
- `serde` — off par défaut ; ajoute `Serialize`/`Deserialize` aux types de config + filaires
  (`CircuitBreakerConfig`, `TimeoutConfig`, `JitterKind`, `BackoffSpec`, `RetrySpec`,
  `ResilienceProfileSpec`). Off ⇒ le crate ne lie aucun code serde.

---

## 🔭 Observabilité

Événements `tracing` aux transitions d'état : transition de circuit (`INFO` `prev`/`next`), circuit
déclenché (`WARN` `+failures`), sonde échouée (`WARN`), retry planifié (`WARN`
`attempt`/`max_attempts`/`delay_ms`), timeout de requête (`WARN` `timeout_ms`). Pas encore d'export de
métriques OTel — à ajouter via le crate `telemetry`.

Alertes service suggérées : transition `CircuitOpen` ⇒ critique ; HalfOpen→Open répétés sans
récupération ⇒ critique ; taux `MaxRetriesExhausted` ⇒ warn ; taux `Timeout` > 1% ⇒ warn.

---

## 🧪 Tests

```bash
cargo test   -p resilience                 # unit tests, no external deps
cargo test   -p resilience --features serde # wire (de)serialization
cargo clippy -p resilience --all-targets
```

Bibliothèque pure in-process — pas de Docker, DB, ni broker. En modifiant le moteur, préserver : config
échantillonnée une fois par op (jamais re-chargée en cours de décision), les swaps ne réinitialisent
jamais l'état live, et les futures boxed `Send` ne doivent pas tenir une valeur non-`Send` à travers un
`.await`.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. Le circuit trip immédiatement au premier appel.**
Un `CircuitBreakerLayer` construit précédemment (qui possède l'`Arc<StateMachine>`) est réutilisé avec un
état persisté. Construire une couche **fraîche** au démarrage ; ne pas la stocker dans un
`once_cell`/`static` sauf si vous voulez explicitement un état inter-redémarrage.

**2. Les retries amplifient la charge au lieu de la réduire (pic 3–4× sur une panne aval).**
`JitterKind::None`/`Equal` sur une flotte ⇒ retries en lockstep. Utiliser `JitterKind::Full` (le défaut).
Vérifier aussi que `CircuitBreakerLayer` enveloppe `RetryLayer` — sinon le circuit ne compte que l'appel
initial et trip trop tard.

**3. Erreur de compilation `Req: Clone` sur `RetryLayer`.**
`RetryService` clone la requête pour la ré-émettre par tentative. Les structs `prost`/tonic dérivent
`Clone`, mais des wrappers custom non — dériver `Clone`, ou ne passer que le proto interne cloneable dans
la région de retry.
