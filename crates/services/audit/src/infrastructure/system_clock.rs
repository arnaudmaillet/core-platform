//! The production [`Clock`] adapter — reads the real wall clock.

use chrono::{DateTime, Utc};

use crate::application::port::Clock;

/// Reads `Utc::now`. The fake (`FixedClock`) pins an instant for deterministic
/// tests; this is the only place real time enters the service.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
