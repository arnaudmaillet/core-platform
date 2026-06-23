//! Pure server-side rate-limiting mechanism — the ingress mirror of the `resilience` crate.
//!
//! `resilience` protects a *caller* from a slow/failing downstream (client side); `traffic`
//! protects a *server* from too many inbound callers (server side). Both share the
//! externalized catalog+bindings model: `infra-config` parses the `[traffic]` section and
//! resolves bindings into the [`TrafficProfile`] handles this crate produces.
//!
//! This crate is deliberately transport-agnostic: it owns the limiter, the config types,
//! and a `check(key) -> `[`TrafficDecision`] decision — no `tonic`, no `http`, no identity
//! plumbing. The gRPC layer that extracts a key from a request and translates a `Throttle`
//! into `RESOURCE_EXHAUSTED` lives in `transport`, where the tonic/http coupling belongs.
//!
//! # State locality (Step 1)
//!
//! Only [`Mode::Local`] — in-process, per-replica `governor` (GCRA) limiters — is enforced.
//! [`Mode::Distributed`] is parsed for forward-compatibility but rejected by `infra-config`
//! validation until the Redis-lease backend ships (Step 2).

pub mod config;
pub mod profile;

pub use config::{BackendError, Mode, Scope, TrafficConfig, TrafficDecision};
pub use profile::{TrafficProfile, TrafficProfileSpec};
