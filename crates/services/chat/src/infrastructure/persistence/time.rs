use chrono::{DateTime, Utc};
use scylla::value::CqlTimestamp;

/// Converts a ScyllaDB `timestamp` to a chrono UTC datetime. An out-of-range
/// value falls back to the Unix epoch rather than panicking — defensive against
/// corrupt rows on the read path.
pub(crate) fn to_utc(ts: CqlTimestamp) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ts.0).unwrap_or(DateTime::UNIX_EPOCH)
}

/// Converts a chrono UTC datetime to a ScyllaDB `timestamp`.
pub(crate) fn to_cql(dt: DateTime<Utc>) -> CqlTimestamp {
    CqlTimestamp(dt.timestamp_millis())
}
