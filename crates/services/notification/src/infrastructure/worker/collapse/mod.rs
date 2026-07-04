pub mod buffer;
pub mod key;

pub use buffer::CollapseBuffer;
pub use key::CollapseKey;

/// Global sorted-set key listing collapse windows due to be flushed, scored by
/// their flush deadline (Unix ms). This is intentionally a single unsharded key
/// so the flush worker can discover *all* due windows with one `ZRANGEBYSCORE`.
///
/// Because it cannot co-locate with the per-window keys (which shard by
/// `(target, subject, kind)`), it must NEVER be written from inside a multi-key
/// Lua script alongside a window key — doing so triggers CROSSSLOT. The schedule
/// is updated with standalone `ZADD`/`ZREM` commands instead.
pub const SCHEDULE_KEY: &str = "notification:window_schedule";
