// crates/post/src/domain/types/duration_seconds.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub struct DurationSeconds(u32);

impl DurationSeconds {
    pub const MAX_DURATION: u32 = 3600;

    pub fn try_new(seconds: u32) -> Result<Self> {
        let duration = Self(seconds);
        duration.validate()?;
        Ok(duration)
    }

    pub fn from_raw(seconds: u32) -> Self {
        Self(seconds)
    }

    pub fn value(&self) -> u32 {
        self.0
    }

    pub fn is_short_format(&self) -> bool {
        self.0 <= 60
    }

    pub fn to_timestamp_string(&self) -> String {
        let minutes = self.0 / 60;
        let seconds = self.0 % 60;
        format!("{:02}:{:02}", minutes, seconds)
    }
}

impl ValueObject for DurationSeconds {
    fn validate(&self) -> Result<()> {
        if self.0 > Self::MAX_DURATION {
            return Err(Error::validation("duration_seconds", "..."));
        }
        Ok(())
    }
}

// --- CONVERSIONS ---

impl TryFrom<u32> for DurationSeconds {
    type Error = Error;
    fn try_from(seconds: u32) -> Result<Self> {
        Self::try_new(seconds)
    }
}

impl TryFrom<i32> for DurationSeconds {
    type Error = Error;
    fn try_from(seconds: i32) -> Result<Self> {
        if seconds < 0 {
            return Err(Error::validation(
                "duration_seconds",
                "Duration cannot be negative",
            ));
        }
        Self::try_new(seconds as u32)
    }
}

impl From<DurationSeconds> for u32 {
    fn from(duration: DurationSeconds) -> Self {
        duration.0
    }
}

impl From<&DurationSeconds> for u32 {
    fn from(duration: &DurationSeconds) -> Self {
        duration.0
    }
}

impl std::fmt::Display for DurationSeconds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s", self.0)
    }
}
