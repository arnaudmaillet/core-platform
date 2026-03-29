// crates/shared-kernel/src/infrastructure/grpc/utils.rs

use chrono::{DateTime, NaiveDate, Utc};
use prost_types::Timestamp;

pub trait ChronoTimestampExt {
    fn to_proto(&self) -> Timestamp;
}
pub trait ProtoTimestampExt {
    fn to_utc_datetime(&self) -> Option<DateTime<Utc>>;
    fn to_naive_date(&self) -> Option<NaiveDate>;
}

impl ChronoTimestampExt for DateTime<Utc> {
    fn to_proto(&self) -> Timestamp {
        Timestamp {
            seconds: self.timestamp(),
            nanos: self.timestamp_subsec_nanos() as i32,
        }
    }
}

impl ProtoTimestampExt for Timestamp {
    fn to_utc_datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(self.seconds, self.nanos as u32)
    }

    fn to_naive_date(&self) -> Option<NaiveDate> {
        self.to_utc_datetime().map(|dt| dt.date_naive())
    }
}