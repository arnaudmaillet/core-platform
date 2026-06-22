-- Migration 002: per-profile chronological notification feed
--
-- Partition key : target_profile_id   — all rows for one recipient in one partition.
-- Clustering    : (created_at DESC, notification_id ASC)
--                 created_at DESC  → newest first; notification_id (UUID v7)
--                 breaks timestamp ties deterministically.
--
-- Compaction: TWCS with 7-day windows.
--   Notifications are pure append-on-write and TTL-expired at 90 days.
--   TWCS segments SSTables by time, allowing entire expired windows to be
--   dropped without generating O(n) tombstones — critical for high-volume
--   celebrity partitions.
--
-- Collapse fields (sender_count, sample_sender_ids):
--   Written by the write-collapse pipeline. sender_count > 1 means the row
--   represents multiple actors coalesced in a time window.
--   The client displays "X, Y and N others reacted" using these fields.
--   No profile handle or avatar is stored here — the client hydrates those
--   from the profile service on demand.
--
-- is_read is the only column ever UPDATEd after initial insert.
-- A separate Redis key (notification:read_horizon:{profile_id}) covers
-- bulk "mark all as read" without touching individual rows.

CREATE TABLE IF NOT EXISTS notification.notifications_by_profile (
    target_profile_id   uuid,
    created_at          timestamp,
    notification_id     uuid,

    notification_kind   tinyint,
    subject_kind        tinyint,
    subject_id          uuid,

    sender_profile_id   uuid,
    sender_count        int,
    sample_sender_ids   list<uuid>,

    is_read             boolean,

    PRIMARY KEY ((target_profile_id), created_at, notification_id)
)
WITH CLUSTERING ORDER BY (created_at DESC, notification_id ASC)
 AND compaction = {
       'class'                  : 'TimeWindowCompactionStrategy',
       'compaction_window_unit' : 'DAYS',
       'compaction_window_size' : 7
     }
 AND default_time_to_live = 7776000
 AND gc_grace_seconds     = 86400
 AND comment = 'Per-profile activity feed. 90-day TTL. Cursor: (created_at, notification_id).';
