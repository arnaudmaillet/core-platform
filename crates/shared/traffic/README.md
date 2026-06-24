# `traffic` — Server-Side Ingress Rate Limiting (pure mechanism)

## 🎯 Overview & Service Role

`traffic` is the **server-side mirror of [`resilience`](../resilience)**: where resilience
protects a *caller* from a slow/failing downstream (client side), `traffic` protects a
*server* from too many inbound callers (ingress). Both use the externalized catalog model —
[`infra-config`](../infra-config) parses the `[traffic]` section and resolves bindings into
the [`TrafficProfile`] handles this crate produces; the gRPC layer that applies them lives in
[`transport`](../transport).

This crate is deliberately **transport-agnostic and pure**: it owns the limiter, the config
types, and a `check(key) -> TrafficDecision` decision — no `tonic`, no `http`, no identity
plumbing, no Redis.

## 📐 Model

| Knob | Values | Meaning |
|---|---|---|
| `rps` / `burst` | `u32` | Sustained rate + bucket capacity (token-bucket / GCRA via `governor`). |
| `scope` | `per_method` \| `per_caller` | Key dimension. `per_caller` keys on an edge-mesh identity (resolved in `transport`); falls back to per-method when absent. |
| `mode` | `local` \| `distributed` | `local` = in-process per-replica (fleet limit ≈ rps × replicas). `distributed` = fleet-global budget via a [`QuotaBackend`] (see [`traffic-redis`](../traffic-redis)). |
| `enforce` | `bool` (default `true`) | `false` = **shadow**: charge the cell and observe the would-throttle, but admit. The observe-before-enforce rollout rail. |
| `on_backend_error` | `fail_open` \| `fail_closed` | Distributed only: degrade to the local limiter, or reject, when the backend is unreachable. |

- **Config behind `ArcSwap`**: `rps`/`burst`/`scope`/`enforce` hot-reload. A quota change
  rebuilds the keyed limiter (state reset) **only when `rps`/`burst` actually change**;
  other changes (e.g. `enforce`) never reset live buckets.
- **`QuotaBackend`** is the distributed seam (async trait). `local` mode never touches it;
  the implementation lives in `traffic-redis` so this crate — and `transport` — link no Redis.

## 🔌 Key API

```rust
let profile: TrafficProfile = registry.profile_for("/post.PostService/CreatePost");
match profile.check("/post.PostService/CreatePost|alice") {  // key = method[|identity]
    TrafficDecision::Allow => { /* forward */ }
    TrafficDecision::Throttle { retry_after } => { /* shed */ }
}
profile.prune();        // drop idle keys (bound memory for per_caller)
profile.quota();        // the global Quota for distributed enforcement
```

## 📦 Integration

```toml
traffic = { workspace = true }                 # pure
# traffic = { workspace = true, features = ["serde"] }   # infra-config enables this to parse [traffic]
```

Bind profiles to gRPC methods in `[traffic]` and install the layer via
`GrpcServerBuilder::with_traffic`. See the
[binary rollout checklist](../infra-config/README.md#-binary-bootstrap--rollout-checklist).

## 🛠️ Local Development

```bash
cargo test -p traffic            # limiter behaviour (admit/shed, isolation, hot-reload)
cargo clippy -p traffic -- -D warnings
```
