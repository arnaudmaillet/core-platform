//! Externalized configuration layer for the pure [`resilience`] middleware crate.
//!
//! `resilience` owns the *mechanism* (Tower layers + the serde-able wire types). This
//! crate owns the *policy plumbing* that the mechanism must not depend on: file IO, TOML
//! parsing, validation, fleet bindings, and `notify`-based hot-reload. Keeping it separate
//! is what lets `resilience` stay free of `notify`/`toml`/filesystem concerns.
//!
//! # Flow
//!
//! ```text
//! infrastructure.toml ──load──▶ InfrastructureConfig ──resolve──▶ ResilienceRegistry
//!                                       ▲                                │
//!                                       │ notify file event              │ profile_for("post-command")
//!                                  spawn_watcher ──validate──▶ registry.apply()   ▼
//!                                  (single writer, fail-closed)        ResilienceProfile ──▶ Tower layers
//! ```
//!
//! See [`schema`] for the TOML shape and `examples/infrastructure.toml` for a full sample.

pub mod error;
pub mod registry;
pub mod schema;
pub mod watcher;

pub use error::ConfigError;
pub use registry::ResilienceRegistry;
pub use schema::{InfrastructureConfig, ResilienceSection};
pub use watcher::{load_from_path, spawn_watcher};
