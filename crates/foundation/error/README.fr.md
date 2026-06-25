---
i18n:
  source: ./README.md
  source_sha256: 842ac1277aeb5c41b0fc8e2c5e296a538836fe6482ffc9ad9477aaa62537a6ee
  translated_at: 2026-06-25
  status: complete
---
> 🇫🇷 Traduction française — la version **anglaise** [`README.md`](./README.md) fait foi.
> En cas de divergence, l'anglais prime. Les contrats (codes d'erreur, variables
> d'environnement, signatures, identifiants) sont volontairement laissés en anglais.

# `error` — Le contrat d'erreur distribuée de la plateforme : un trait, une forme de sérialisation, zéro fuite

> **Fiche crate**
>
> | | |
> |---|---|
> | **Rôle** | `foundation` — contrat d'erreur à l'échelle du workspace, sans logique métier |
> | **Package** | `error` (dir : `crates/foundation/error`) |
> | **Consommé par** | `cqrs`, `transport`, `resilience`, et chaque crate de service |
> | **Dépend de** | `thiserror`, `tracing`, `http`, `uuid`, `chrono` (axum seulement en dev-deps) |
> | **Stabilité** | contrat stable (`error_code` est de l'API publique) |
> | **Feature flags** | aucun |
> | **Propriétaire** | `<TODO: équipe>` · `<TODO: #canal-slack>` |

---

## 🎯 Vue d'ensemble & rôle

`error` est la fondation, à l'échelle du workspace, de la gestion d'erreurs structurée, observable et
distribuée. Il fournit le contrat, le vocabulaire et les primitives de sérialisation qui permettent à
chaque microservice de définir son **propre** enum d'erreur indépendamment, tout en garantissant une
sortie uniforme, sûre pour le client et riche en télémétrie à la frontière de la plateforme.

**Frontière architecturale** — il ne définit **aucune logique métier ni domaine**. Il n'a pas d'E/S
réseau ni d'état, donc il ne peut jamais être la cause d'une défaillance en cascade. Tous les réglages
opérationnels (format de log, échantillonnage, alerting) vivent dans le bootstrap du service consommateur.

**Objectifs fondamentaux :** un trait / une forme JSON / un vocabulaire de sévérité quel que soit le
service qui a levé l'erreur ; zéro fuite d'information (`trace_id`/`span_id` restent dans les logs,
n'atteignent jamais les clients) ; préservation du type de bout en bout (`DistributedError<E>` garde le
type concret — pas de `Box<dyn Error>`) ; divulgation progressive (seules deux méthodes sont
obligatoires sur `AppError`).

---

## 📐 Architecture & décisions clés

```
Service error enum (e.g. AuthError) ──implements──► AppError  (+ blanket IntoApiResponse)
        │ wrapped by
        ▼
DistributedError<E> { error: E (typed), context: ErrorContext }
        │ .log() ──► tracing event (trace/span ids INSIDE)
        │ into_api_response()
        ▼
ApiErrorResponse  ──JSON──►  API client   (NO trace_id / span_id)
```

Les quatre piliers : **Contrat** (`AppError` + `IntoApiResponse`), **Vocabulaire** (`Severity`),
**Contexte** (`ErrorContext` + `DistributedError<E>`), **Format filaire** (`ApiErrorResponse` +
`into_api_response`).

- **Enveloppe typée, pas d'effacement** — `DistributedError<E>` garde le type concret, donc les
  consommateurs gardent le pattern-matching complet. L'alternative (`Box<dyn Error>`) a été rejetée :
  elle détruit le type sur lequel le service doit brancher.
- **Divulgation à deux niveaux** — seules `error_code()` et `http_status()` sont obligatoires sur
  `AppError` ; tout le reste a un défaut sûr en production, donc ajouter une méthode ne casse jamais les
  implémenteurs.
- **Pas de heap sur le chemin chaud** — les méthodes d'`AppError` renvoient `&'static str` ; l'enveloppe
  est allouée sur la pile jusqu'à ce que le service la renvoie. Sur les chemins sensibles à la latence,
  les services peuvent utiliser `Box<DistributedError<E>>` pour garder le `Result` petit
  (`#[allow(clippy::result_large_err)]`).
- **La fuite est structurellement impossible** — `trace_id`/`span_id` vivent sur `ErrorContext`
  (journalisés) mais sont *absents* d'`ApiErrorResponse`. Tant que vous construisez la réponse via les
  helpers, ils ne peuvent pas atteindre un client.

---

## 🔌 API publique & contrat

```rust
pub trait AppError: std::error::Error + Send + Sync + 'static {
    fn error_code(&self) -> &'static str;        // "AUTH_TOKEN_EXPIRED" — rename = breaking change
    fn http_status(&self) -> StatusCode;
    // optional, production-safe defaults:
    fn severity(&self) -> Severity { Severity::Medium }
    fn is_retryable(&self) -> bool { false }
    fn category(&self) -> &'static str { "UNKNOWN" }
    fn user_facing_message(&self) -> &'static str { "An error occurred." }
}

pub trait IntoApiResponse: AppError + Sized { fn to_api_response(&self, ctx: &ErrorContext) -> ApiErrorResponse; }
impl<E: AppError> IntoApiResponse for E {}        // blanket — do NOT override

pub struct ErrorContext { /* request_id, trace_id, span_id, service_name, timestamp, metadata */ }
pub struct DistributedError<E: AppError> { pub error: E, pub context: ErrorContext }
impl<E: AppError> DistributedError<E> { pub fn new(error: E, context: ErrorContext) -> Self; pub fn log(&self); }

pub struct ApiErrorResponse { /* error_code, message, request_id, service, severity, retryable, category, timestamp, details */ }
pub fn into_api_response<E: AppError>(err: &DistributedError<E>) -> ApiErrorResponse;  // trace/span stripped
```

`Severity` est le vocabulaire d'urgence unifié (`Critical`/`High` font « page » ; `Medium` par défaut ;
`Low`/`Info` non). Il implémente `Ord` comme **`Critical < High < Medium < Low < Info`** (« plus
d'urgence = valeur plus basse »).

> **Contrat de stabilité :** `error_code` fait partie de l'API publique — clients et dashboards s'en
> servent comme clé. Tout renommage est un changement cassant nécessitant une migration versionnée. Le
> blanket impl `IntoApiResponse` ne doit pas être surchargé.

---

## 📦 Intégration

```toml
[dependencies]
error = { workspace = true }
thiserror = { workspace = true }   # recommended for ergonomic error definitions
```

```rust
// 1. domain enum  2. impl AppError  3. (axum) newtype for the orphan rule  4. use `?`
pub struct ApiError(pub DistributedError<AuthError>);
impl From<DistributedError<AuthError>> for ApiError { fn from(e: DistributedError<AuthError>) -> Self { ApiError(e) } }
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let err = self.0;
        err.log();                          // structured log w/ trace+span ids
        let status = err.error.http_status();
        (status, Json(into_api_response(&err))).into_response()   // safe client JSON
    }
}
```

Une version pleinement compilable vit dans [`examples/auth_service.rs`](examples/auth_service.rs) :
`cargo run -p error --example auth_service`.

---

## ⚙️ Configuration & feature flags

Bibliothèque sans état — aucune variable d'environnement, aucun thread d'arrière-plan, aucune feature
cargo. `axum` est une **dev-dependency uniquement** (pour l'exemple) ; ajouter `error` à un service
non-axum n'apporte aucune dépendance axum.

---

## 🔭 Observabilité

`DistributedError::log()` émet un événement `tracing` au niveau `severity().log_level()` avec les champs :
`request_id`, `trace_id`, `span_id`, `service`, `severity`, `error_code`, `category`, `retryable`,
`error.message`. Corréler par `request_id` (visible client) ou `trace_id` + `span_id` (logs seulement).

Alertes suggérées : tout `severity = Critical|High` ⇒ page ; pic du taux d'événements `error_code` > 5×
la baseline ⇒ page ; taux `retryable = true` soutenu ⇒ warn (instabilité amont).

---

## 🗂️ Organisation des modules

```
src/
├── traits.rs    AppError + IntoApiResponse (blanket)
├── context.rs   ErrorContext + DistributedError<E> + .log()
└── http.rs      ApiErrorResponse + into_api_response (client wire format)
```

---

## 🧪 Tests

```bash
cargo test   -p error                  # unit tests inline in source
cargo clippy -p error --all-targets
cargo run    -p error --example auth_service
```

Aucun service externe requis — bibliothèque Rust pure.

---

## 🚨 Pièges / FAQ

> Les arêtes vives. Une entrée par piège réel.

**1. `impl IntoResponse for DistributedError<MyError>` ne compile pas.**
Règle d'orphelin — `IntoResponse` (axum) et `DistributedError` (ce crate) sont tous deux étrangers à
votre service. Paramétrer avec un type local n'aide pas (`DistributedError` n'est pas `#[fundamental]`).
Encapsuler dans un newtype possédé par le service `ApiError(pub DistributedError<MyError>)` (voir
l'exemple).

**2. `.log()` ne produit aucune sortie.**
Aucun subscriber `tracing` n'est installé ; les événements sont silencieusement écartés. En installer un
dans `main`/le setup de test (`tracing_subscriber::fmt::init()` en dev ; un subscriber JSON en prod).

**3. `trace_id` / `span_id` ont fuité dans une réponse client.**
Le service a sérialisé `ErrorContext` ou `DistributedError` directement. À ne **jamais** faire — toujours
construire le corps client via `into_api_response(&err)` (ou `err.to_api_response(&ctx)`), qui les retire.

**4. Ajouter une méthode à `AppError` a cassé les implémenteurs.**
Les nouvelles méthodes DOIVENT porter un défaut conservateur (`traits.rs`) — seule façon rétro-compatible.
Mettre ensuite à jour `DistributedError::log()` / `ApiErrorResponse` si le champ doit apparaître.
