# `auth-context` — Domain & Functional Contract

> The inbound security translator: it answers *"who is this caller?"* — converting an opaque Bearer token into a typed principal and propagating it through the async call-stack.

> **Domain Card**
>
> | | |
> |---|---|
> | **Shared capability** | Inbound authentication: JWT verification (token → typed `CurrentPrincipal`) + task-local identity propagation |
> | **Layer** | `platform` — the security translator at a service's inbound boundary |
> | **Subdomain class** | **Generic** — standard OIDC/JWT verification; leverage is provider-agnosticism + zero-thread propagation |
> | **Primary abstraction(s)** | `JwtDecoder` + `CurrentPrincipal<C>` + `ClaimsExtractor<C>` (`auth_context`) |
> | **Footprint** | IO/stateful — a background JWKS refresher task + a cache; verification is pure CPU |
> | **Failure posture** | **fail-closed on a bad token** (reject) but **fail-soft on a flaky IdP** (stale keys keep working) |
> | **Depends on** | `jsonwebtoken`, `tokio`, `reqwest` (JWKS), `tracing`, `uuid`, `cqrs` (optional) |
> | **Consumed by** | service inbound boundaries / the gateway (anything authenticating a request) |
> | **Decision log** | none — rationale in [`README §Architecture`](../README.md); related: [`ADR-0005 (auth)`](../../../../docs/adr/0005-auth-federated-idp-with-platform-edge-tokens.md) |

---

## 1. Technical Capability & Non-Goals &nbsp;·&nbsp; CORE

**Capability.** `auth-context` is the fleet's authority for **request authentication**: it answers
**"who is this caller, and how do I make that identity available everywhere downstream without threading it
through every function signature?"** It does authentication (verify + extract + propagate), not authorization
policy and not transport.

**The hard problem.** Token verification must be a *pure CPU* hot-path operation (no network per request), yet
public keys rotate and live at an OIDC JWKS endpoint, and the resulting identity must reach deep business
logic without polluting signatures. `auth-context` fetches+caches keys in a background task (stale keys keep
working through an IdP blip), keeps claims-extraction pluggable per IdP, and binds the principal to a
`tokio::task_local!`.

**Non-goals — what this crate deliberately does NOT do:**
- ❌ Authorize (decide what a principal may do) → that is the service's policy.
- ❌ Issue or refresh tokens / broker the IdP → that is the `auth` service (`ADR-0005`).
- ❌ Own transport / the inbound HTTP layer → it converts a token a caller already extracted.
- ❌ Propagate identity across `tokio::spawn` automatically → re-bind with `with_principal` inside the task.

---

## 2. Ubiquitous Language &nbsp;·&nbsp; CORE

| Term | Meaning in this crate | Code symbol |
|---|---|---|
| Principal | The typed authenticated identity (id, tenant, permissions, raw claims) | `CurrentPrincipal<C>`, `PrincipalId`, `Permission` |
| Claims extractor | The per-IdP strategy mapping raw claims → a principal | `ClaimsExtractor<C>`, `OidcClaimsExtractor` |
| JWKS cache / refresher | The cached public keys + the background refresh loop | `JwksCache`, `JwksRefresher`, `JwksClient` |
| Decoder | The verify-then-extract entry point | `JwtDecoder` |
| Task-local principal | Identity bound to the async call-stack, read without threading | `with_principal`, `current_principal` |

---

## 3. Public Model & Contract Surface &nbsp;·&nbsp; CORE

| Element | Kind | Contract / invariant boundary it guards |
|---|---|---|
| `JwtDecoder<C, E>` | verifier | `decode(token)` = header `kid` lookup → `jsonwebtoken::decode` (sig + `exp`/`nbf`/`iss`/`aud`) → extract |
| `CurrentPrincipal<C>` | value type | The raw claims travel as generic `C` (type-safe identity) |
| `ClaimsExtractor<C>` | trait (seam) | The single strategy point — a new IdP/flow is a new extractor, not a decoder fork |
| `JwksCache` | cache | O(1) `RwLock` read on the hot path; `replace` swaps the key set atomically |
| `with_principal` / `current_principal` | propagation | `task_local!` binding; **not** inherited across `spawn` |
| `AuthError` | enum | The precise rejection reason (`InvalidSignature`, `TokenExpired`, `UnknownKid`, …) |

---

## 4. Ownership & Architectural Boundaries &nbsp;·&nbsp; CORE

**This crate owns:**
- Verification (signature + standard claims), claims-extraction strategy, the JWKS cache + background
  refresher, and task-local propagation.

**This crate deliberately does NOT own / must NOT link:**

| Concern | Lives in | Why the edge points that way |
|---|---|---|
| Authorization policy (what a principal may do) | each service | This crate ends at identity, not permission decisions |
| Token issuance / IdP brokerage | `auth` service | Issuance is a System-of-Record concern (`ADR-0005`) |
| Envelope injection into `cqrs` | gated behind `cqrs-integration` | Keeps the `cqrs` edge optional |

**The "do-not-depend-on" list:** never a service crate; the `cqrs` dependency is feature-gated
(`cqrs-integration`) so non-CQRS callers link no `cqrs`.

---

## 5. Invariants & Contract Rules &nbsp;·&nbsp; CORE

| # | Invariant | Enforced at | On violation |
|---|---|---|---|
| I1 | Verification makes no network call (keys come from cache) | `JwtDecoder::decode` | hot-path latency / IdP coupling |
| I2 | A flaky IdP does not fail the hot path (stale keys keep working) | `JwksRefresher` backoff loop | (degraded only when no key matches) |
| I3 | The task-local principal is **not** propagated across `tokio::spawn` | `task_local!` semantics | missing identity in the sub-task — re-bind |
| I4 | A new IdP/flow is a new `ClaimsExtractor`, not a decoder change | strategy seam | a fork of the decoder |
| I5 | `iss`/`aud` validation is on by default in prod | `AuthContextConfig` | token-confusion risk if disabled |

---

## 6. Control Flow & Lifecycle &nbsp;·&nbsp; DEEP

**Startup.** `JwksRefresher::spawn` warms the cache immediately, then loops: fetch → `replace(keys)`, delay =
`refresh_interval`; on error → WARN, delay = backoff (1s → `max_backoff`, ×2). The refresher guard must outlive
the process.

**Hot path — per request.** `decode_header()` reads the `kid` (no crypto) → `JwksCache::get(kid)` (O(1)
`RwLock` read) → `jsonwebtoken::decode` (RS256/ES256 + `exp`/`nbf`/`iss`/`aud`, with `AUTH_CLOCK_SKEW_SECS`
leeway) → `ClaimsExtractor::extract` → `CurrentPrincipal<C>`. Then `with_principal(p, fut)` binds the
task-local for the request's duration; `inject_into_span()` (and optionally `inject_into_envelope`) surface the
identity to observability/CQRS.

---

## 7. Crate Coupling (dependency-graph slice) &nbsp;·&nbsp; DEEP

| Neighbour crate | Direction | Pattern | Mechanism | What breaks if it changes |
|---|---|---|---|---|
| service inbound boundaries / gateway | downstream | Published Contract | `decode` + `with_principal` | request authentication |
| `cqrs` | upstream (optional) | Open-Host (extends) | `inject_into_envelope` (`cqrs-integration`) | identity in command metadata |
| `auth` service | peer (runtime) | Customer/Supplier | consumes JWKS the IdP publishes | key rotation / token validity |

> **Stability seam:** `CurrentPrincipal<C>`, `ClaimsExtractor<C>`, `JwtDecoder`, and `AuthError` are public API;
> the `task_local!`-not-across-spawn rule is a contract callers must respect.

---

## 8. Emitted Signals & Side-Effects &nbsp;·&nbsp; DEEP

| Signal | Kind | Emitted when | Who observes |
|---|---|---|---|
| JWKS cache refreshed | `tracing` INFO (`key_count`, `next_refresh_secs`) | a successful background refresh | auth dashboards |
| refresh failed | `tracing` WARN (`error`, `retry_after_secs`) | a JWKS fetch error | P2 alert if > 3/min |
| key loaded / undecodable key skipped | `tracing` DEBUG/WARN | parsing the JWKS set | key-rotation monitoring |

Side effects: one outbound HTTP fetch per refresh interval (not per request); a `task_local!` binding per
request. `JwksCache::is_empty()` for > 30s is a P1 (can authenticate nothing).

---

## 9. Decisions & Rationale &nbsp;·&nbsp; DEEP

| Decision | Where recorded | Status |
|---|---|---|
| JWKS cached + background-refreshed; verification is pure CPU on the hot path | [`README §Architecture`](../README.md) | Accepted |
| Pluggable `ClaimsExtractor<C>` for provider-agnosticism | [`README §Architecture`](../README.md) | Accepted |
| Task-local propagation instead of signature-threading | [`README §Architecture`](../README.md) | Accepted |
| Authentication-only; issuance/brokerage is the `auth` service | [`ADR-0005`](../../../../docs/adr/0005-auth-federated-idp-with-platform-edge-tokens.md) | Accepted |

---

## 10. Classification & Evolution &nbsp;·&nbsp; DEEP

- **Classification:** Generic — standard OIDC/JWT verification; leverage is provider-agnosticism and
  zero-thread propagation.
- **Stability:** stable contract.
- **Volatility:** low — new IdPs/flows land as new `ClaimsExtractor`s, not surface changes.
- **Deferred capabilities:** service-account / machine-to-machine claim mapping (`azp`/`client_id` → user_id)
  is a custom extractor, not yet a built-in.
