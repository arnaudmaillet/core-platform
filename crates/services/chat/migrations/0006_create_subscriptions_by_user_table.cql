-- Migration 0006: reverse subscription index — "my subscriptions" / membership probe
--
-- Partition key: subscriber_id — all conversations a user subscribes to live in
--   one partition (bounded by how many channels a single user follows, which is
--   small), enabling a single-partition "list my subscriptions" scan and an O(1)
--   "am I subscribed to conversation X?" point read on the audience read path.
--
-- Clustering: conversation_id ASC — deterministic point probe and paginated list.
--
-- Written/deleted in lockstep with subscriptions_by_conversation. The pair is
--   not transactional (Scylla has no multi-partition ACID); a best-effort
--   reconciliation job (future) repairs rare divergence, consistent with the
--   platform's other denormalized dual-write indexes.
--
-- Compaction: LCS — subscribe/unsubscribe deletes; same rationale as 0005.
CREATE TABLE IF NOT EXISTS chat.subscriptions_by_user (
    subscriber_id   uuid,
    conversation_id uuid,
    subscribed_at   timestamp,
    PRIMARY KEY ((subscriber_id), conversation_id)
) WITH CLUSTERING ORDER BY (conversation_id ASC)
  AND compaction  = {'class': 'LeveledCompactionStrategy'}
  AND compression = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Reverse index for list-my-subscriptions and the O(1) audience membership probe.';
