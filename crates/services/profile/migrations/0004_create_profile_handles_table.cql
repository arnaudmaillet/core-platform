-- tombstoned_at NULL = active claim; non-NULL = handle released but under 30-day reservation.
-- Background GC job reclaims rows where tombstoned_at < (now - 30d).
CREATE TABLE IF NOT EXISTS profile.profile_handles (
    handle         text,
    profile_id     uuid,
    account_id     uuid,
    created_at     timestamp,
    tombstoned_at  timestamp,
    PRIMARY KEY (handle)
) WITH compression = {'sstable_compression': 'LZ4Compressor'};
