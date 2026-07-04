-- Spatial index: maps each (H3 tile × resolution) to the posts it contains.
--
-- Composite partition key (h3_index, resolution):
--   Prevents hot partitions on viral urban tiles by distributing each resolution
--   level of the same tile across different CQL partitions. A city center at
--   resolution 9 may contain thousands of rows; collapsing all resolutions into
--   one partition would create a prohibitively hot shard.
--
-- Clustering: (published_at DESC, post_id ASC):
--   published_at drives TWCS bucketing — SSTables align with daily write windows,
--   enabling full-SSTable eviction 48 h after the window closes (zero row-level
--   tombstones). DESC ordering ensures recency-first range scans for cold-start
--   recovery without a secondary sort.
--
-- ScyllaDB role: cold-start recovery and authoritative durability only.
--   All hot-path spatial queries are served by Redis ZSETs. This table is read
--   only during service restart (Redis warm-up) or on Redis cache miss.
--
-- virality_score is intentionally absent from the clustering key: scores change
--   on every engagement event, and mutating a clustering key requires a
--   DELETE + INSERT cycle. Score-based filtering is delegated to Redis.
--
-- Per-row TTL:   Set via USING TTL ? on each INSERT. Default 172 800 s (48 h).
--   Premium creators may supply a custom retention_secs (e.g. 7 × 86 400 s).
-- TWCS window:   1 day. A post's TTL ≤ 48 h, so at most 2 windows cover all
--   live rows. When an entire window expires, ScyllaDB drops the SSTable without
--   scanning individual rows.
CREATE TABLE IF NOT EXISTS geo_discovery.posts_by_tile (
    h3_index     bigint,       -- Raw H3 cell index cast to signed int64 (bit 63 always 0)
    resolution   tinyint,      -- H3 resolution stored as raw integer: 5, 7, or 9
    published_at timestamp,    -- Write-time bucket, drives TWCS window placement
    post_id      uuid,
    PRIMARY KEY ((h3_index, resolution), published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 1
  }
  AND compression      = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Spatial tile index at H3 resolutions 5/7/9. TWCS 1-day windows align with the 48 h default TTL for full-SSTable eviction. Score filtering delegated to Redis ZSETs.';
