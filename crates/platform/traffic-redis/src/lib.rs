//! Redis-lease distributed backend for the `traffic` rate limiter (Step 2).
//!
//! Implements [`traffic::QuotaBackend`] so `distributed` profiles enforce a *fleet-global*
//! budget **without a Redis round-trip per request**: each replica leases a chunk of the
//! global per-window budget and serves it locally, only crossing to Redis when its chunk is
//! exhausted (or to discover the window is fully spent). Backend I/O is therefore amortized
//! over `burst` requests per key per replica; a fully-spent window is cached locally so a
//! flood of over-budget requests does not hammer Redis.
//!
//! # Layering
//!
//! ```text
//! QuotaBackend (trait, in `traffic`)
//!   └─ RedisLeaseBackend
//!        ├─ LeaseBook      — local per-key lease cache + windowed-budget algorithm (pure)
//!        └─ ClaimSource    — atomic "lease N tokens" seam
//!             └─ RedisClaimSource — one Lua script (single key → cluster-slot-safe)
//! ```
//!
//! The algorithm in [`LeaseBook`] is transport- and Redis-agnostic and unit-tested against
//! an in-memory [`ClaimSource`]; [`RedisClaimSource`] is the thin live-Redis implementation.
//!
//! # Failure handling
//!
//! A claim failure surfaces as [`traffic::QuotaError`]; the transport layer maps it to the
//! profile's `on_backend_error` policy (degrade to the local limiter, or reject). Requests
//! served from an existing local lease never touch Redis, so a Redis blip only affects
//! requests that need a refill.

pub mod claim;
pub mod lease;
pub mod redis;

pub use claim::ClaimSource;
pub use lease::{window_budget, LeaseBook};
pub use redis::{RedisClaimSource, RedisLeaseBackend};
