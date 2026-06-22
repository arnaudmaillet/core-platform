/// Runtime configuration for the notification service.
///
/// All fields are loaded from environment variables at startup with
/// safe production defaults. No hot-reload — changing thresholds requires
/// a service restart to prevent mid-flight divergence between workers.
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Reaction count per 5-minute window that classifies a subject as "hot"
    /// and activates the Redis cross-batch collapse window.
    pub hot_subject_threshold: u32,

    /// TTL of the Redis cross-batch collapse window in seconds.
    /// CollapseFlushWorker settles windows older than this.
    pub collapse_window_secs: u64,

    /// How often the CollapseFlushWorker polls for settled windows.
    pub collapse_flush_interval_secs: u64,

    /// Maximum notifications delivered per (target, subject, kind) tuple per hour.
    /// Writes exceeding this limit are discarded and counted in metrics.
    pub max_notifications_per_subject_per_hour: u32,

    /// Maximum value stored in the Redis unread counter.
    /// The badge displays "99+" above this cap.
    pub unread_cap: i64,

    /// TTL for cached post-author lookups (`notification:pa:{post_id}`).
    pub post_author_cache_ttl_secs: u64,

    /// TTL for cached comment-author lookups (`notification:ca:{comment_id}`).
    pub comment_author_cache_ttl_secs: u64,

    /// TTL for cached social-graph block lookups.
    pub block_cache_ttl_secs: u64,

    /// Maximum number of notifications returned per ListNotifications page.
    pub max_page_size: i32,

    /// tokio::sync::broadcast channel capacity per active streaming profile.
    /// Receivers that fall behind by this many messages receive `Lagged`.
    pub stream_buffer_size: usize,

    /// Maximum number of sample sender UUIDs stored per collapse bucket.
    pub max_sample_senders: usize,

    /// TTL (seconds) for idempotency claim keys (`notification:dedupe:...`).
    /// Must exceed the worst-case Kafka redelivery window so a retried event is
    /// recognised as a duplicate and does not double-increment the unread counter.
    pub dedupe_ttl_secs: u64,
}

impl NotificationConfig {
    pub fn from_env() -> Self {
        Self {
            hot_subject_threshold: env_u32("NOTIFICATION_HOT_SUBJECT_THRESHOLD", 100),
            collapse_window_secs: env_u64("NOTIFICATION_COLLAPSE_WINDOW_SECS", 30),
            collapse_flush_interval_secs: env_u64("NOTIFICATION_COLLAPSE_FLUSH_INTERVAL_SECS", 30),
            max_notifications_per_subject_per_hour: env_u32(
                "NOTIFICATION_MAX_PER_SUBJECT_PER_HOUR",
                3,
            ),
            unread_cap: env_i64("NOTIFICATION_UNREAD_CAP", 99),
            post_author_cache_ttl_secs: env_u64(
                "NOTIFICATION_POST_AUTHOR_CACHE_TTL_SECS",
                604_800,
            ),
            comment_author_cache_ttl_secs: env_u64(
                "NOTIFICATION_COMMENT_AUTHOR_CACHE_TTL_SECS",
                259_200,
            ),
            block_cache_ttl_secs: env_u64("NOTIFICATION_BLOCK_CACHE_TTL_SECS", 300),
            max_page_size: env_i32("NOTIFICATION_MAX_PAGE_SIZE", 50),
            stream_buffer_size: env_usize("NOTIFICATION_STREAM_BUFFER_SIZE", 256),
            max_sample_senders: env_usize("NOTIFICATION_MAX_SAMPLE_SENDERS", 5),
            dedupe_ttl_secs: env_u64("NOTIFICATION_DEDUPE_TTL_SECS", 86_400),
        }
    }
}

fn env_u32(var: &str, default: u32) -> u32 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(var: &str, default: u64) -> u64 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_i32(var: &str, default: i32) -> i32 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_i64(var: &str, default: i64) -> i64 {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(var: &str, default: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
