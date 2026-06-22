use uuid::Uuid;

/// Maximum number of time buckets a single history page will walk before giving
/// up, bounding worst-case latency on sparse/old history. At the default 24-hour
/// bucket this is a ~90-day look-back per page request.
pub const MAX_BUCKET_WALK: i32 = 90;

/// Number of hash buckets the audience subscription set is spread across in
/// `subscriptions_by_conversation`. Fixed: changing it would re-key existing
/// rows. Sized so a viral channel's millions of subscribers split into evenly
/// loaded, individually scannable partitions.
pub const SUBSCRIPTION_BUCKETS: i64 = 64;

/// Maps a message timestamp (epoch ms) to its `messages_by_conversation` time
/// bucket: `floor(created_at_ms / window_ms)`. The same function is used by the
/// writer (current bucket) and the reader (cursor/floor bucket), so a message is
/// always read from the partition it was written to.
pub fn message_bucket(created_at_ms: i64, bucket_hours: u32) -> i32 {
    let window_ms = bucket_hours.max(1) as i64 * 3_600_000;
    (created_at_ms.max(0) / window_ms) as i32
}

/// Maps a subscriber to its `subscriptions_by_conversation` hash bucket. Derived
/// purely from `subscriber_id`, so subscribe and unsubscribe always target the
/// same partition without a lookup.
pub fn subscription_bucket(subscriber_id: Uuid) -> i32 {
    let bytes = subscriber_id.as_bytes();
    let hi = u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]);
    (hi % SUBSCRIPTION_BUCKETS as u64) as i32
}
