//! Externalized configuration & hot-reload for fleet infrastructure (the IO/policy layer
//! the pure middleware crates must not depend on).
//!
//! The pure crates own the *mechanism* — `resilience` (client-side Tower layers), `traffic`
//! (server-side rate limiter) — plus their serde-able wire types. This crate owns the
//! *policy plumbing* they must stay free of: file IO, TOML parsing, validation, fleet
//! bindings, and `notify`-based hot-reload. It is **multi-tenant**: each infrastructure
//! category is a `[section]` sharing one catalog shape ([`catalog`]), one watcher, and one
//! fail-closed reload path. Today: `[resilience]`, `[cache]`, `[traffic]`.
//!
//! # Flow
//!
//! ```text
//! infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ InfraRegistry
//!                                       ▲                          ├─ ResilienceRegistry
//!                                  notify file event               ├─ CacheRegistry
//!                                  spawn_watcher ──reload──▶ apply()└─ TrafficRegistry
//!                                  (single writer, fail-closed, all-sections-or-nothing)
//! ```
//!
//! See [`schema`] for the TOML shape and `examples/infrastructure.toml` for a full sample.

pub mod cache;
pub mod catalog;
pub mod error;
pub mod infra;
pub mod registry;
pub mod reload;
pub mod schema;
pub mod traffic;
pub mod watcher;

pub use cache::{CacheConfig, CacheProfile, CacheProfileSpec, CacheRegistry, CacheSection};
pub use catalog::Catalog;
pub use error::ConfigError;
pub use infra::InfraRegistry;
pub use registry::ResilienceRegistry;
pub use reload::Reloadable;
pub use schema::{InfrastructureConfig, ResilienceSection};
pub use traffic::{TrafficRegistry, TrafficSection};
pub use watcher::{load_from_path, spawn_watcher};
