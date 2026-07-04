-- Durable read-model projection: stores all rendering metadata needed to
-- populate a map card without fan-out to services/post or services/profile.
--
-- Anti-N+1 contract: every field required by the BFF map renderer is stored
-- here. No secondary gRPC calls are needed to display a fully hydrated card.
--
-- LCS (not TWCS): access is exclusively point-reads by post_id. There are no
--   range scans on this table. TWCS would increase read amplification for
--   point lookups without providing any SSTable-drop benefit (since partitions
--   are unique per post_id and do not share SSTable windows). LCS minimises
--   read amplification for the mixed point-read / UPDATE workload.
--
-- virality_score: mutable column. Updated by the score_updater worker on every
--   engagement.score_updated Kafka event. UPDATE is in-place (no clustering key
--   mutation). This is the authoritative score for cold-start Redis reconstruction.
--
-- h3_index_r7: canonical tile at resolution 7. Stored so that deep-link
--   navigation can center the map on the post's tile without decoding the post's
--   original lat/lng (not stored here to keep the projection lean).
--
-- expires_at: redundant with per-row TTL but stored explicitly so the cold-start
--   reconstruction worker can compute the remaining TTL for ZADD pipelines.
--
-- Per-row TTL:   Set via USING TTL ? on each INSERT. Default 172 800 s (48 h).
CREATE TABLE IF NOT EXISTS geo_discovery.map_post_cards (
    post_id           uuid,
    author_id         uuid,
    author_handle     text,
    author_avatar_url text,
    thumbnail_url     text,
    caption           text,      -- Post caption, served on the Focus-mode read path
    h3_index_r7       bigint,    -- Canonical resolution-7 tile for deep-link centering
    virality_score    float,     -- Mutable, updated by score_updater worker
    published_at      timestamp,
    expires_at        timestamp, -- Explicit expiry for cold-start TTL computation
    author_tier       tinyint,   -- 0=Standard, 1=Premium, 2=VIP (mutable via TierSyncWorker)
    PRIMARY KEY (post_id)
) WITH compaction      = {'class': 'LeveledCompactionStrategy'}
  AND compression      = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Durable denormalized card projection. LCS for point-read workload. Per-row USING TTL supports premium extended retention. virality_score and author_tier are mutable via UPDATE.';
