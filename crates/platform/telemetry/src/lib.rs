//! Centralised observability bootstrap for the core-platform workspace.
//!
//! Initialise the full telemetry pipeline — structured logging, distributed
//! tracing (OTLP/gRPC), and metrics (Prometheus or OTLP) — with a single
//! [`init`] call.  Keep the returned [`TelemetryGuard`] alive for the
//! lifetime of the process; dropping it flushes all in-flight spans and logs.

pub mod config;
pub mod control;
pub mod error;
pub mod guard;
pub mod init;
pub mod log;
pub mod metrics;
pub mod trace;

pub use config::TelemetryConfig;
pub use control::TelemetryControl;
pub use error::TelemetryError;
pub use guard::TelemetryGuard;
pub use init::init;
pub use trace::config::SamplingStrategy;
