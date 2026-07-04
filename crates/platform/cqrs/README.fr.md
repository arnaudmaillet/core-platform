---
i18n:
  source: ./README.md
  source_sha256: dca02732614c340c66c2e5eee2e88ee4684c7e8f504bad4b419968085c0d412c
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `cqrs` — Bus de commandes/requêtes in-process à coût nul avec pipeline de middleware typé

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `platform` — la couche de dispatch applicatif (statique, in-process) |
> | **Package** | `cqrs` (dir : `crates/platform/cqrs`) |
> | **Consommé par** | chaque service (bus de commandes/requêtes) ; `validation` & `auth-context` l'étendent |
> | **Dépend de** | `error`, `validate-core`, `uuid`, `chrono`, `dashmap`, `tracing` |
> | **Stabilité** | contrat stable |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`cqrs` est la couche de dispatch applicatif : un **Command Bus** et un **Query Bus** in-process,
pleinement statiques et hautes performances, qui routent les opérations de domaine vers leur unique
handler enregistré **sans dispatch dynamique sur le chemin chaud**. Chaque message voyage sur un
`Envelope<T>` portant `message_id` / `correlation_id` / `causation_id` pour la propagation de trace de
bout en bout.

**Frontière architecturale** — séparation stricte écriture/lecture imposée par le système de types : les
commandes mutent et renvoient `()` ; les requêtes renvoient des données typées et ne portent pas
d'effets de bord. Il n'y a aucun moyen d'écrire l'état via le chemin de requête. C'est un bus in-process
pur — pas de queue, pas de réseau, pas d'env.

---

## 📐 Architecture & décisions clés

```
gRPC/Kafka handler ─► Envelope::new(correlation_id, payload)
  ▼ LoggingCommandBus (outermost) ─► TracingCommandBus ─► IdempotencyCommandBus
    ─► InMemoryCommandBus (TypeId→handler map) ─► TypedHandlerBridge<H,C> ─► Arc<H>.handle(envelope)
  ▼ Result<(), CqrsError>   (queries: Result<Q::Response, CqrsError>)
```

- **Routage par TypeId, sans réflexion, sans vtable** — les handlers s'enregistrent par `TypeId` au
  démarrage ; le dispatch est un `HashMap::get` + un `Box::new(envelope)` pour franchir la frontière
  d'effacement, puis le handler s'exécute via un pont compilé statiquement. Le seul `dyn` est dans les
  `ErasedCommandHandler`/`ErasedQueryHandler` **scellés `pub(crate)`** ; le downcast ne peut échouer car
  la clé `TypeId` garantit le type.
- **Pas d'`async_trait`** — handlers/bus utilisent du RPIT natif (`-> impl Future<…> + Send + '_`), donc
  le pipeline est monomorphisé de bout en bout sans futures boxed dans la chaîne de wrappers. `BoxFuture`
  n'apparaît **que** dans les ponts effacés.
- **Modules middleware `pub(crate)` + `pub use` ciblés** — évite la collision de glob entre les types de
  couche commande et requête tout en gardant les noms publics propres.
- **L'idempotence ne marque que sur `Ok`** — `mark_processed` ne s'exécute que sur succès, donc un handler
  en échec laisse le message non marqué et réessayable en sécurité. `InMemoryIdempotencyStore` (DashMap)
  est non borné — le remplacer par un store TTL (Redis `SET NX EX`) dans les services longue durée.

---

## 🔌 API publique & contrat

```rust
pub struct Envelope<T> { pub message_id: Uuid, pub correlation_id: Uuid, pub causation_id: Option<Uuid>,
                         pub issued_at: DateTime<Utc>, pub metadata: HashMap<String,String>, pub payload: T }
impl<T> Envelope<T> {
    pub fn new(correlation_id: Uuid, payload: T) -> Self;            // fresh causal chain (ingress)
    pub fn new_caused_by<P>(parent: &Envelope<P>, payload: T) -> Self; // inherits correlation + metadata, sets causation
    pub fn with_metadata(self, k: impl Into<String>, v: impl Into<String>) -> Self;
    pub fn map<U, F: FnOnce(T)->U>(self, f: F) -> Envelope<U>;
}

pub trait Command: Send + Sync + 'static {}                          // (supertrait: validate_core::Validate)
pub trait Query:   Send + Sync + 'static { type Response: Send + Sync + 'static; }
pub trait CommandHandler<C: Command>: Send + Sync + 'static { type Error: AppError; fn handle(&self, e: Envelope<C>) -> impl Future<Output=Result<(), Self::Error>> + Send + '_; }
pub trait QueryHandler<Q: Query>:     Send + Sync + 'static { type Error: AppError; fn handle(&self, e: Envelope<Q>) -> impl Future<Output=Result<Q::Response, Self::Error>> + Send + '_; }
pub trait CommandBus: Send + Sync { fn dispatch<C: Command>(&self, e: Envelope<C>) -> impl Future<Output=Result<(), CqrsError>> + Send + '_; }       // not object-safe
pub trait QueryBus:   Send + Sync { fn dispatch<Q: Query>(&self, e: Envelope<Q>)   -> impl Future<Output=Result<Q::Response, CqrsError>> + Send + '_; }

pub enum CqrsError { HandlerNotFound { type_name: &'static str }, DuplicateRegistration { type_name: &'static str }, Handler(BoxedDynAppError) }
// impl AppError — Handler(e) delegates error_code/http_status/severity/… to the original handler error.

pub trait CommandLayer<S> { type Service; fn layer(&self, inner: S) -> Self::Service; }   // + QueryLayer<S>
pub trait IdempotencyStore: Send + Sync + 'static { fn is_processed(&self, id: Uuid) -> impl Future<Output=bool>+Send+'_; fn mark_processed(&self, id: Uuid) -> impl Future<Output=()>+Send+'_; }
```

Couches livrées : `TracingLayer` (`info_span!` par dispatch), `LoggingLayer` (start/complete +
`elapsed_ms`/`error.code`), `IdempotencyLayer<Store>` (command-only, dedup par `message_id`). Codes
`CqrsError` : `HandlerNotFound`→`CQRS_HANDLER_NOT_FOUND`/500,
`DuplicateRegistration`→`CQRS_DUPLICATE_REGISTRATION`/500, `Handler(e)`→délègue.

> **Contrat :** les traits de dispatch ne sont **pas object-safe** (`dispatch<C>` générique) — tenir le
> type de bus concret (ou son `Arc`). Le bus décoré final est un type concret, p. ex.
> `LoggingCommandBus<TracingCommandBus<IdempotencyCommandBus<InMemoryCommandBus>>>`.

---

## 📦 Intégration

```toml
[dependencies]
cqrs = { workspace = true }
```

```rust
// build at startup — register (fails fast on duplicates), then decorate; FIRST .layer() = outermost.
let raw = CommandBusBuilder::new()
    .register::<CreatePostCommand, _>(CreatePostHandler { repo })?
    .build();                                   // InMemoryCommandBus (Arc, Clone, immutable)
let command_bus = MiddlewarePipeline::new(raw)
    .layer(IdempotencyLayer::new(InMemoryIdempotencyStore::new()))
    .layer(TracingLayer).layer(LoggingLayer).build();

// dispatch from a gRPC endpoint:
bus.dispatch(Envelope::new(correlation_id, CreatePostCommand { title })).await?;
// causal chaining inside a handler:
bus.dispatch(Envelope::new_caused_by(&incoming, PublishNotificationCommand { user_id })).await?;
```

Les requêtes suivent la même forme avec `QueryBusBuilder` + `.query_layer(...)` (pas d'`IdempotencyLayer`
— les requêtes sont naturellement idempotentes). Middleware custom = implémenter
`CommandLayer<S>`/`QueryLayer<S>` et `CommandBus`/`QueryBus` pour votre wrapper.

---

## ⚙️ Configuration & feature flags

Bibliothèque in-process pure — pas de variables d'environnement, pas de features cargo. Prérequis (hors
du ressort de ce crate) : `telemetry::init()` avant le premier dispatch utilisant
`TracingLayer`/`LoggingLayer`, sinon les événements sont écartés.

---

## 🔭 Observabilité

Span `TracingLayer` `cqrs.command.dispatch` / `cqrs.query.dispatch` : `otel.kind=INTERNAL`,
`message.type`, `message.id`, `correlation.id`. `LoggingLayer` : start + complete/failed avec
`elapsed_ms`, `error`, `error.code`. Chemin chaud = 1× `HashMap::get` (O(1), sans verrou) + 1×
`Box::new` ; clone de bus = `Arc::clone`.

Alertes suggérées : `cqrs.command.dispatch` p99 > 50ms ⇒ warn ; taux d'erreur handler > 1% ⇒ critique ;
croissance RSS non bornée ⇒ warn (`InMemoryIdempotencyStore` non borné).

---

## 🧪 Tests

```bash
cargo test   -p cqrs                 # fully in-process, no external deps
cargo clippy -p cqrs --all-targets
```

En ajoutant une couche livrée, préserver les invariants du moteur : **pas d'`async_trait`** (RPIT natif) ;
`BoxFuture` uniquement pour un nouveau pont effacé, jamais dans les wrappers middleware ; le corps de
`dispatch` est un unique `async move {}` / `.instrument(span)` sans allocation sur le chemin commun.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `CqrsError::HandlerNotFound` à l'exécution.**
Le type de commande n'a jamais été `register`é avant `.build()` (ligne omise, ou le bus a été construit
dans un scope différent de celui où vous dispatchez). Auditer la chaîne du builder ; envisager un
smoke-test de démarrage qui dispatche une commande sonde.

**2. `CqrsError::DuplicateRegistration` au démarrage.**
`register::<C, _>` a été appelé deux fois pour le même `C` (détecté tôt au build). Retirer le doublon ;
pour un vrai fan-out, router via un handler agrégateur unique.

**3. Le RSS croît sans borne sur plusieurs jours.**
`InMemoryIdempotencyStore` n'évince jamais (chaque `message_id` de 16 octets est conservé). Redémarrer sur
une cadence (réplicas sans état) ou implémenter `IdempotencyStore` sur Redis `SET NX EX <ttl>` (~24h
correspond aux fenêtres at-least-once typiques) et le brancher au démarrage.

**4. J'ai essayé de stocker un `&dyn CommandBus` et ça ne compile pas.**
Les traits de dispatch ne sont pas object-safe (`dispatch<C>` est générique). Tenir le type de bus décoré
concret ou son `Arc` au lieu d'un trait object.
