-- Materialized fan-out cold-store for Standard/Premium author posts.
--
-- Partition key: profile_id (follower / feed owner)
--   Each user's feed is a single partition. Reads during cold-start are
--   single-partition token-range scans with no ALLOW FILTERING required.
--
-- Clustering: (published_at DESC, post_id ASC)
--   DESC ordering returns the newest feed items first without a secondary sort.
--   post_id (UUID v7) breaks same-millisecond timestamp ties deterministically.
--
-- author_id: stored for O(partition) unfollow-prune scans.
--   On follow.deleted, the PruneFollowWorker issues a full-partition scan
--   filtered by author_id in application code. Because this partition is
--   per-follower (counts in hundreds, not millions) this is safe.
--
-- Compaction: TWCS 30-day windows.
--   Feed items are pure appends. Default TTL = 30 days. TWCS segments
--   SSTables by creation window, allowing entire expired windows to be
--   dropped as a unit without generating O(n) tombstones — critical for
--   high-velocity profiles where daily post volume is high.
--
-- This table is NEVER on the hot read path. Redis timeline:feed:{id} serves
-- all active requests. This table is read only during Redis cold-start
-- reconstruction or when the warm flag (timeline:warm:{profile_id}) is absent.
CREATE TABLE IF NOT EXISTS timeline.feed_items_by_profile (
    profile_id   uuid,          -- follower (feed owner)
    published_at timestamp,     -- post publication timestamp; TWCS bucket driver
    post_id      uuid,          -- post token (UUID v7 from services/post)
    author_id    uuid,          -- denormalized for unfollow-prune scans
    PRIMARY KEY ((profile_id), published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 30
  }
  AND default_time_to_live = 2592000
  AND compression          = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds     = 86400
  AND comment = 'Materialized fan-out feed for Standard/Premium authors. Cold-start source only. TWCS 30-day windows. Hot path served by Redis timeline:feed:{profile_id}.';
