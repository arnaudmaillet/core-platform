-- Dual-purpose per-author reverse index.
--
-- Role A — VIP at-read merge source (cold-start for timeline:vip:{author_id}):
--   When a VIP author's Redis registry ZSET is evicted, this table is scanned
--   to reconstruct timeline:vip:{author_id}. The author_tier column allows the
--   cold-start hydration worker to route VIP vs Standard correctly without an
--   external tier lookup.
--
-- Role B — Follow backfill source (Standard/Premium authors only):
--   On follow.created, the FollowCreatedWorker queries this table for the
--   followee's most recent posts (LIMIT 100 DESC) and injects them into the
--   new follower's Redis feed and feed_items_by_profile ScyllaDB rows.
--   This keeps the new follower's feed immediately populated without requiring
--   a full-history replay.
--
-- Written for ALL authors on post.published (one row per post — cheap writes).
--
-- author_tier: mutable via UPDATE when a tier-change event arrives.
--   Stored as tinyint (i8): 0=Standard, 1=Premium, 2=Vip.
--   The timeline service does not own this value; it mirrors it from the
--   post.published payload where geo-discovery has already denormalized it.
--
-- Compaction: TWCS 30-day windows aligned with feed_items_by_profile.
--   VIP registry Redis TTL is 7 days; at-read merge never looks back further.
--   Entries naturally expire after 30 days in ScyllaDB, after which they are
--   irrelevant for both backfill (> 30 days of history is not backfilled) and
--   VIP merge (> 30 days is beyond the Redis hot window).
CREATE TABLE IF NOT EXISTS timeline.posts_by_author (
    author_id    uuid,
    published_at timestamp,     -- TWCS bucket driver; DESC ordering for recency-first scans
    post_id      uuid,          -- UUID v7 from services/post
    author_tier  tinyint,       -- 0=Standard, 1=Premium, 2=Vip; mirrors geo-discovery
    PRIMARY KEY ((author_id), published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 30
  }
  AND default_time_to_live = 2592000
  AND compression          = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds     = 86400
  AND comment = 'Per-author post reverse index. Dual role: VIP at-read cold-start source and Standard/Premium follow-backfill source. TWCS 30-day windows. author_tier is mutable.';
