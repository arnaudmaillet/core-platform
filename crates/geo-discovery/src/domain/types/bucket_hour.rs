// crates/geo-discovery/src/domain/types/bucket_hour.rs

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Error, Result, ValueObject};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BucketHour(i64);

impl BucketHour {
    pub const MILLIS_IN_DAY: i64 = 24 * 60 * 60 * 1000;

    pub fn from_timestamp(timestamp_millis: i64) -> Self {
        let truncated = (timestamp_millis / Self::MILLIS_IN_DAY) * Self::MILLIS_IN_DAY;
        Self(truncated)
    }

    pub fn value(&self) -> i64 {
        self.0
    }

    pub fn to_date_time(&self) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(self.0).unwrap()
    }
}

impl ValueObject for BucketHour {
    fn validate(&self) -> Result<()> {
        if self.0 <= 0 {
            return Err(Error::validation("bucket_hour", "Invalid timestamp bucket"));
        }
        Ok(())
    }
}
