// crates/shared-kernel/src/core/identity/versioning.rs

use chrono::{DateTime, Utc};

pub trait Versioned {
    fn version(&self) -> u64;
    fn updated_at(&self) -> DateTime<Utc>;
    fn record_change(&mut self);
}
