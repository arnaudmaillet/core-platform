pub mod config;
pub mod error;
pub mod health;
pub mod pool;
pub mod routing;
pub mod transaction;

// ── Backward-compatible re-exports (unchanged symbols) ───────────────────────
pub use config::PostgresConfig;
pub use error::StorageError;
pub use health::health_check;
pub use pool::builder::PgPoolBuilder;
pub use transaction::manager::{PgTransaction, TransactionManager};

// ── Topology-aware additions ──────────────────────────────────────────────────
pub use config::{ShardedPostgresConfig, TopologyConfig};
pub use health::health_check_cluster;
pub use pool::builder::{PgClusterBuilder, TopologyBuilder};
pub use routing::{ShardCluster, ShardId, ShardKey};
