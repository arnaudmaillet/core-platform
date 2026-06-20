# auth-context — Provider-agnostic JWT validation & task-local identity propagation

## 🎯 Overview & Service Role

`auth-context` is the platform's **security translator**: it sits at every service's inbound boundary, converts an opaque Bearer token into a strongly-typed [`CurrentPrincipal<C>`], and makes that identity available throughout the entire async call-stack without threading it through every function signature.

**Core technical objectives:**

- **Zero network overhead per request** — public keys are fetched once from the OIDC JWKS endpoint by a background Tokio task and stored in a lock-optimised in-memory cache. Every token verification is a pure CPU operation on the hot path.
- **Provider agnosticism** — supports Keycloak (`realm_access.roles`), Auth0 (`permissions` array), Okta (`groups`), and any OIDC-compliant issuer via the pluggable [`ClaimsExtractor<C>`] strategy trait.
- **Type-safe identity** — raw claims travel with the principal as a generic type parameter `C`, giving business logic full access to provider-specific fields without a second decode pass.
- **Transparent propagation** — `tokio::task_local!` storage integrates with the CQRS `Envelope<T>` metadata map and `tracing` spans with two one-liner calls.

---

## 📐 Architecture & Concepts

### Verification pipeline (per request)

```
  Bearer token string
        │
        ▼
  decode_header()          ← extract kid — no crypto yet
        │
        ▼
  JwksCache::get(kid)      ← O(1), shared RwLock read
        │
        ▼
  jsonwebtoken::decode()   ← RS256 / ES256 signature + exp/nbf/iss/aud
        │
        ▼
  ClaimsExtractor::extract ← raw C → CurrentPrincipal<C>
        │
        ▼
  with_principal(p, fut)   ← bind to tokio::task_local!
        │
        ▼
  inject_into_span()       ← enrich tracing span
  inject_into_envelope(e)  ← stamp Envelope<T>::metadata
```

### Background JWKS refresh loop

```
  ┌─ JwksRefresher::spawn() ──────────────────────────────────┐
  │                                                           │
  │  loop {                                                   │
  │    sleep(delay)                 ← 0 on first iteration   │
  │    match JwksClient::fetch()                              │
  │      Ok(keys) → JwksCache::replace(keys)  delay = interval│
  │      Err(e)   → WARN log          delay = backoff (2×)   │
  │  }                                backoff ≤ max_backoff  │
  └───────────────────────────────────────────────────────────┘
```

### Resilience guarantees

| Scenario | Behaviour |
|---|---|
| JWKS endpoint temporarily unreachable | Stale keys remain in cache; exponential backoff (1 s → `max_backoff`) |
| Key rotation at IdP | New key picked up within one `refresh_interval`; tokens with old `kid` return `UnknownKid` |
| Cache empty at startup | First fetch is immediate (zero delay); requests arriving before first fetch get `UnknownKid` |
| Decoding CPU burst | `JwtDecoder::decode` is sync-on-the-executor; no `spawn_blocking` needed for ≤ 4096-bit RSA |
| `tokio::spawn` isolation | Task-local principal is NOT propagated to spawned sub-tasks — explicit `with_principal()` required |

---

## 🔌 Public Interfaces & API Contract

### Core types

```rust
/// Opaque platform user identifier (wraps the raw JWT `sub` string).
pub struct PrincipalId(pub String);

/// A single normalised permission token (e.g. `"posts:write"`, `"ROLE_ADMIN"`).
pub struct Permission(pub String);

/// Canonical in-process identity extracted from a verified JWT.
pub struct CurrentPrincipal<C> {
    pub user_id:     PrincipalId,
    pub tenant_id:   Option<String>,
    pub permissions: Vec<Permission>,
    pub raw_claims:  C,              // provider-specific; preserved verbatim
}
```

### Strategy trait

```rust
pub trait ClaimsExtractor<C>: Send + Sync + 'static {
    fn extract(&self, raw: C) -> Result<CurrentPrincipal<C>, AuthError>;
}
```

### JWKS cache

```rust
impl JwksCache {
    pub fn new() -> Self;
    pub async fn get(&self, kid: &str) -> Option<DecodingKey>;
    pub async fn replace(&self, keys: HashMap<String, DecodingKey>);
    pub async fn len(&self) -> usize;
    pub async fn is_empty(&self) -> bool;
}
```

### JWT decoder

```rust
impl<C, E: ClaimsExtractor<C>> JwtDecoder<C, E> {
    pub fn new(config: &AuthContextConfig, cache: JwksCache, extractor: E) -> Self;
    pub async fn decode(&self, token: &str) -> Result<CurrentPrincipal<C>, AuthError>;
}
```

### Task-local context API

```rust
pub fn with_principal<P, Fut>(principal: Arc<P>, future: Fut) -> impl Future<Output = Fut::Output>
where P: AnyPrincipal + 'static, Fut: Future;

pub fn current_principal() -> Option<Arc<dyn AnyPrincipal>>;

pub fn inject_into_span();                                // always available

#[cfg(feature = "cqrs-integration")]
pub fn inject_into_envelope<T>(envelope: &mut cqrs::Envelope<T>);
```

### Error variants

```rust
pub enum AuthError {
    InvalidSignature,
    TokenExpired,
    TokenNotYetValid,
    MissingKid,
    UnknownKid(String),
    JwksUnavailable(String),
    MalformedToken(String),
    InvalidAudience,
    InvalidIssuer,
    ClaimsExtractionFailed(String),
}
```

---

## 📦 Integration & Usage

### `Cargo.toml`

```toml
[dependencies]
auth-context = { path = "../../crates/shared/auth-context" }
# or with CQRS envelope injection:
auth-context = { path = "../../crates/shared/auth-context", features = ["cqrs-integration"] }
```

### Standard bootstrap pattern

```rust
use std::sync::Arc;
use auth_context::{
    AuthContextConfig, JwksCache, JwksClient, JwksRefresher, JwtDecoder,
    OidcClaimsExtractor, OidcExtractorConfig, with_principal, inject_into_span,
};

#[tokio::main]
async fn main() {
    let config = Arc::new(AuthContextConfig {
        jwks_url: std::env::var("JWKS_URL").expect("JWKS_URL required"),
        expected_issuer:  Some(std::env::var("OIDC_ISSUER").expect("OIDC_ISSUER required")),
        expected_audience: Some(std::env::var("OIDC_AUDIENCE").expect("OIDC_AUDIENCE required")),
        ..Default::default()
    });

    let cache = JwksCache::new();
    let client = JwksClient::new(&config.jwks_url, config.fetch_timeout);

    // Warm the cache immediately, then refresh on schedule.
    let _refresher = JwksRefresher::spawn(
        client,
        cache.clone(),
        config.refresh_interval,
        config.max_backoff,
    );

    let decoder = Arc::new(JwtDecoder::new(
        &config,
        cache,
        OidcClaimsExtractor::new(OidcExtractorConfig::default()),
    ));

    // Inside a request handler:
    let bearer_token = extract_bearer_from_header(/* ... */);

    let principal = Arc::new(decoder.decode(&bearer_token).await?);

    with_principal(principal, async {
        inject_into_span();            // enriches the current tracing span
        // inject_into_envelope(&mut env); // with feature = "cqrs-integration"
        handle_request().await
    }).await
}
```

---

## ⚙️ Configuration & Runtime Environment

| Variable | Required | Default | Description |
|---|---|---|---|
| `JWKS_URL` | **Yes** | — | Full JWKS endpoint URL (e.g. `https://idp/realms/x/protocol/openid-connect/certs`) |
| `OIDC_ISSUER` | Recommended | `None` | Expected `iss` claim value; disabling issuer check is not recommended for production |
| `OIDC_AUDIENCE` | Recommended | `None` | Expected `aud` claim value; disabling is acceptable only on fully-trusted internal networks |
| `AUTH_REFRESH_INTERVAL_SECS` | No | `300` | JWKS cache refresh period in seconds |
| `AUTH_MAX_BACKOFF_SECS` | No | `60` | Maximum exponential backoff on JWKS fetch failure |
| `AUTH_CLOCK_SKEW_SECS` | No | `5` | Leeway applied to `exp` and `nbf` checks |
| `AUTH_FETCH_TIMEOUT_SECS` | No | `10` | Per-request HTTP timeout for JWKS fetches |

### Cargo feature flags

| Feature | Default | Effect |
|---|---|---|
| `cqrs-integration` | off | Enables `inject_into_envelope()` and a dep on `crates/shared/cqrs` |

---

## 📈 Telemetry, Performance & Metrics

### Execution prerequisites

- Tokio multi-thread runtime (`#[tokio::main]` or `Runtime::new()`).
- A reachable JWKS endpoint at startup is not strictly required — the refresher backs off gracefully — but requests will return `UnknownKid` until the first successful fetch.

### Emitted tracing events

| Level | Event | Key fields |
|---|---|---|
| `INFO` | JWKS cache refreshed | `key_count`, `next_refresh_secs` |
| `WARN` | JWKS refresh failed | `error`, `retry_after_secs` |
| `DEBUG` | JWKS key loaded | `kid`, `kty` |
| `WARN` | Undecodable JWKS key skipped | `kid`, `kty`, `reason` |

### Recommended OTel / Prometheus alerts

| Alert | Condition | Severity |
|---|---|---|
| `auth_jwks_fetch_failure` | `WARN` log rate > 3 per minute sustained | **P2** — IdP connectivity issue |
| `auth_unknown_kid_rate` | `UnknownKid` error rate > 1 % of requests | **P3** — possible key rotation lag |
| `auth_token_expired_rate` | `TokenExpired` error rate > 5 % of requests | **P3** — client-side token management issue |
| `auth_cache_empty` | `JwksCache::is_empty()` true for > 30 s | **P1** — service cannot authenticate any request |

---

## 🛠️ Local Development & Contribution

```bash
# Build
cargo build -p auth-context

# Unit + integration tests (no external services needed)
cargo test -p auth-context

# With CQRS integration feature
cargo test -p auth-context --features cqrs-integration

# Lint
cargo clippy -p auth-context -- -D warnings

# Format check
cargo fmt -p auth-context -- --check
```

**No `docker compose` is required.** All tests use in-process RSA key generation and a pre-seeded `JwksCache` — there is no live IdP dependency.

---

## 🚨 Troubleshooting & Runbook

### `UnknownKid` errors on every request immediately after deploy

**Root cause:** The `JwksRefresher` background task has not completed its first fetch yet, or the JWKS endpoint returned a key set that does not include the `kid` embedded in the tokens being issued.

**Mitigation:**
1. Check logs for `"JWKS refresh failed"` WARN entries — the `error` field contains the underlying HTTP error.
2. Verify `JWKS_URL` resolves from inside the container (`curl $JWKS_URL`).
3. Confirm the IdP is issuing tokens with a `kid` that matches a key published in the JWKS document.

---

### `InvalidSignature` errors after key rotation

**Root cause:** The IdP rotated its signing key and issued new tokens before the `JwksRefresher` picked up the new key. The old `DecodingKey` in the cache no longer matches.

**Mitigation:**
1. Reduce `AUTH_REFRESH_INTERVAL_SECS` during the rotation window (e.g. to `30`).
2. Ensure the IdP publishes the new key in JWKS *before* it starts issuing tokens signed with it (standard OIDC rotation protocol).
3. Trigger a manual cache refresh by restarting the process if interval-based recovery is too slow.

---

### `ClaimsExtractionFailed: JWT 'sub' claim is absent or empty`

**Root cause:** The IdP is issuing client-credentials-flow tokens or machine-to-machine tokens where the `sub` claim is absent or set to the client ID rather than a user identifier.

**Mitigation:** Implement a custom `ClaimsExtractor<C>` that maps the client ID (`azp`, `client_id`) to `user_id` for service-account flows, and register it in place of `OidcClaimsExtractor`.
