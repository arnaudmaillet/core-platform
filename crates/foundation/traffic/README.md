# `traffic` — Pure server-side rate-limiting mechanism: the ingress mirror of `resilience`

> **Crate Card**
>
> | | |
> |---|---|
> | **Role** | `foundation` — transport-agnostic ingress rate-limiting mechanism |
> | **Package** | `traffic` (dir: `crates/foundation/traffic`) |
> | **Consumed by** | `transport` (extracts a key, maps `Throttle` → `RESOURCE_EXHAUSTED`), `infra-config` (parses `[traffic]`) |
> | **Depends on** | `arc-swap`, `async-trait`, `governor` (GCRA), `serde` (optional) |
> | **Stability** | evolving (Distributed mode lands in Step 2) |
> | **Feature flags** | `serde` (off by default — pure; `infra-config` turns it on) |
> | **Owner** | `<TODO: team>` · `<TODO: #slack-channel>` |

---

## 🎯 Overview & role

`traffic` is the pure server-side rate-limiting mechanism — the **ingress mirror of `resilience`**.
`resilience` protects a *caller* from a slow/failing downstream (client side); `traffic` protects a
*server* from too many inbound callers (server side). Both share the externalized catalog+bindings
model: `infra-config` parses the `[traffic]` section and resolves bindings into the `TrafficProfile`
handles this crate produces.

**Architectural boundary** — deliberately **transport-agnostic**: it owns the limiter, the config
types, and a `check(key) -> TrafficDecision`. No `tonic`, no `http`, no identity plumbing. The gRPC
layer that extracts a key from a request and translates a `Throttle` into `RESOURCE_EXHAUSTED` lives
in `transport`, where the tonic/http coupling belongs.

---

## 📐 Architecture & key decisions

```
infra-config  ──parses [traffic] (serde on)──►  TrafficProfileSpec ──resolve──►  TrafficProfile
                                                                                      │ check(key)
transport (gRPC): extract key from request ───────────────────────────────────────►  ▼
                  Throttle → RESOURCE_EXHAUSTED        TrafficDecision::Allow | Throttle { retry_after }
```

- **Mirror of `resilience`** — same catalog+bindings shape, opposite direction. Keeping the mechanism
  symmetric means one mental model and one config story for both ingress and egress.
- **Pure by default** — with `serde` off the crate pulls in no serde/derive code (same boundary as
  `resilience`); `infra-config` enables `serde` only where it needs to parse the section.
- **Transport-agnostic** — the crate stops at `check(key) -> TrafficDecision`. Request-key extraction
  and the `RESOURCE_EXHAUSTED` mapping are `transport`'s job, so `traffic` never links tonic/http.
- **State locality (Step 1)** — only `Mode::Local` is enforced: in-process, per-replica `governor`
  (GCRA) limiters. `Mode::Distributed` is *parsed* for forward-compatibility but **rejected by
  `infra-config` validation** until the Redis-lease backend ships (Step 2).

---

## 🔌 Public API & contract

```rust
pub use backend::{Quota, QuotaBackend, QuotaError, DEFAULT_LEASE_MS};
pub use config::{BackendError, Mode, Scope, TrafficConfig, TrafficDecision};
pub use profile::{TrafficProfile, TrafficProfileSpec};

pub enum Mode { Local, Distributed }            // Distributed: parsed, not yet enforced
pub enum Scope { /* keying scope for the limiter */ }
pub enum TrafficDecision { Allow, Throttle { /* retry-after */ } }

impl TrafficProfile {
    pub fn check(&self, key: &str) -> TrafficDecision;   // GCRA decision for this key
    pub fn apply(&self, spec: &TrafficProfileSpec);      // hot-swap config (ArcSwap)
    pub fn prune(&self);                                 // evict idle per-key limiter state
    pub fn key_count(&self) -> usize;
    pub fn enforce(&self) -> bool;
    pub fn scope(&self) -> Scope;
    pub fn mode(&self) -> Mode;
}
```

> **Contract notes:** `check(key)` is the hot path — `Allow` or `Throttle { retry_after }`. `apply`
> hot-swaps the profile's config via `ArcSwap` (driven by `infra-config` hot-reload). Per-key limiter
> state grows with distinct keys; call `prune()` periodically to evict idle keys.

---

## 📦 Integration

```toml
[dependencies]
traffic = { workspace = true }                 # add `features = ["serde"]` only to parse config (infra-config does)
```

```rust
use traffic::{TrafficDecision};

// `transport` owns this glue: derive a key from the request, then:
match profile.check(&key) {
    TrafficDecision::Allow => { /* proceed */ }
    TrafficDecision::Throttle { .. } => { /* return RESOURCE_EXHAUSTED with retry-after */ }
}
```

---

## ⚙️ Configuration & feature flags

No environment variables — config arrives as a `[traffic]` section through `infra-config` (catalog of
`TrafficProfileSpec` + bindings), hot-reloadable via `apply`.

**Feature flags:**
- `serde` — off by default (pure mechanism). `infra-config` enables it to deserialize the `[traffic]`
  section. A service consuming only resolved `TrafficProfile` handles needs it off.

---

## 🧪 Testing

```bash
cargo test   -p traffic                    # GCRA decisions, hot-swap, pruning
cargo test   -p traffic --features serde   # config (de)serialization
cargo clippy -p traffic --all-targets
```

Pure library — no external services (Local mode is in-process).

---

## 🚨 Gotchas / FAQ

> The sharp edges. One entry per real trap.

**1. `Mode::Distributed` in my `[traffic]` config is rejected at boot.**
By design — only `Mode::Local` is enforced in Step 1. Distributed is parsed for forward-compatibility
but `infra-config` validation rejects it until the Redis-lease backend ships (Step 2). Use `Local`.

**2. Limiter memory grows over time.**
Per-key GCRA state accumulates one entry per distinct key. Call `prune()` on a timer to evict idle
keys; check `key_count()` to size the cadence.

**3. I added `traffic` and it pulled in serde unexpectedly / I need config parsing but serde is off.**
The `serde` feature is off by default (keeps the crate pure). Enable `features = ["serde"]` only where
you parse the section — services consuming resolved `TrafficProfile` handles should leave it off.

**4. Looking for the `RESOURCE_EXHAUSTED` mapping or request-key extraction — not here.**
This crate is transport-agnostic and stops at `check(key) -> TrafficDecision`. The tonic/http glue
(key extraction, `Throttle` → `RESOURCE_EXHAUSTED`) lives in `transport`.
