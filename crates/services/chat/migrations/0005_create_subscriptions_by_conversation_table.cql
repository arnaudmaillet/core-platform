-- Migration 0005: the unbounded Audience Plane subscription set — hash-bucketed
--
-- A viral public conversation has MILLIONS of subscribers, so this set is itself
-- a hot-partition risk if keyed by conversation_id alone. We bucket it by a
-- HASH of the subscriber_id (not by time): this spreads subscribers evenly
-- across a fixed number of partitions regardless of subscribe rate, and — unlike
-- a time bucket — makes the unsubscribe DELETE deterministic (the bucket is
-- recomputed from subscriber_id, no lookup needed).
--
-- Partition key: (conversation_id, bucket)
--   bucket = hash(subscriber_id) % SUBSCRIPTION_BUCKETS (64).
--
-- Clustering: subscriber_id ASC — point membership probes within a bucket and
--   ordered, paginated admin/analytics scans.
--
-- This table is NEVER on the message-delivery path: the Audience Plane is
--   fan-out-on-read (the broadcast plane), never fan-out-on-write, so we never
--   iterate this set to deliver a message. It backs admin listing, analytics,
--   and bucketed counting. The hot "am I subscribed?" probe uses
--   subscriptions_by_user instead (single point read).
--
-- Compaction: LCS — subscribe/unsubscribe churn produces deletes; LCS bounds
--   tombstone spread and read amplification better than STCS/TWCS here.
CREATE TABLE IF NOT EXISTS chat.subscriptions_by_conversation (
    conversation_id uuid,
    bucket          int,        -- hash(subscriber_id) % 64; even spread + deterministic delete
    subscriber_id   uuid,
    subscribed_at   timestamp,
    PRIMARY KEY ((conversation_id, bucket), subscriber_id)
) WITH CLUSTERING ORDER BY (subscriber_id ASC)
  AND compaction  = {'class': 'LeveledCompactionStrategy'}
  AND compression = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Hash-bucketed audience subscription set. Never on the delivery path (fan-out-on-read). Admin/analytics listing only.';
