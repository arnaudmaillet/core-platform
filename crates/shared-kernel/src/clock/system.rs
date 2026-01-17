use chrono::{DateTime, Utc};
use crate::clock::Clock;

// crates/shared-kernel/src/clock/system.rs
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> { Utc::now() }
}