//! Live scenarios, grouped by concern. Each mints a fresh tenant so its chain
//! partition is unique and the suite runs in parallel against the shared
//! containers.

mod anchor;
mod chain;
mod idempotency;
mod shred;
