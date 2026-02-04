use crate::clock::Clock;
use chrono::{DateTime, Utc};

// crates/shared-kernel/src/clock/system.rs
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
