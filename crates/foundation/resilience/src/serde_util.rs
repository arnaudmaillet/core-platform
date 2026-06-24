//! Serde adapters shared by the config/wire types.
//!
//! Only compiled when the `serde` feature is enabled, so the core middleware
//! never links serialization machinery it doesn't need.

/// (De)serializes a [`std::time::Duration`] as an integer count of milliseconds.
///
/// Durations are rendered as `{ secs, nanos }` by serde's default impl — unusable
/// in a hand-edited `infrastructure.toml`. This keeps the wire format a flat
/// `*_ms` integer (e.g. `open_duration_ms = 30_000`).
pub mod duration_millis {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u64(d.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_millis(u64::deserialize(d)?))
    }
}
