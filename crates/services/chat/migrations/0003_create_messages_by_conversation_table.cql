-- Migration 0003: the durable message log — time-bucketed
--
-- This is the single most important schema decision in the service. Unlike the
-- per-entity tables elsewhere in the platform (per-profile, per-post), a viral
-- public conversation has an UNBOUNDED message count AND millions of concurrent
-- readers on the same conversation_id. A single-column partition key would be an
-- ever-growing hot partition that saturates its replica set. We therefore use a
-- COMPOSITE, TIME-BUCKETED partition key.
--
-- Partition key: (conversation_id, bucket)
--   bucket = floor(created_at_ms / (CHAT_MESSAGE_BUCKET_HOURS * 3_600_000)).
--   Bounds each partition's size AND migrates the write hotspot across token
--   ranges (and frequently replica sets) over time. Consequence — the central
--   guarantee of the blueprint:
--     * members WRITE to the CURRENT bucket (the live tail);
--     * guests SCROLL history in OLDER buckets (cold partitions);
--   so the write hotspot and the bulk passive-read load are physically
--   segregated by bucket age and cannot collide on the same replicas.
--
-- Clustering: (created_at DESC, message_id ASC)
--   Newest-first without a secondary sort; message_id (UUID v7) breaks
--   same-millisecond ties deterministically. Cursor = (created_at, message_id),
--   identical to the platform's other feed tables.
--
-- Audience watermark: history reads for subscribers/guests add a server-side
--   clustering predicate `AND created_at >= <public_since_ms>`, pushing the
--   public-since visibility filter into ScyllaDB rather than the application.
--
-- Compaction: TWCS, 1-day windows. Messages are append-only and immutable
--   (no edits in v1), so each window seals into read-optimal SSTables — ideal
--   for the cold-history read offload. No default TTL: chat history is retained
--   (retention, if introduced, becomes a per-keyspace policy decision).
--
-- media_ref holds an out-of-band pointer for Media messages; large media is
-- never inlined. body carries Text/System content (empty for Media captions).
CREATE TABLE IF NOT EXISTS chat.messages_by_conversation (
    conversation_id uuid,
    bucket          int,        -- time bucket; partition spreader (see header)
    created_at      timestamp,
    message_id      uuid,       -- UUID v7, time-ordered, dedupe key
    sender_id       uuid,
    content_type    tinyint,    -- 0=Text, 1=Media, 2=System
    body            text,
    media_ref       text,
    reply_to        uuid,       -- threaded reply target; NULL if none
    PRIMARY KEY ((conversation_id, bucket), created_at, message_id)
) WITH CLUSTERING ORDER BY (created_at DESC, message_id ASC)
  AND compaction = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 1
  }
  AND compression      = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Time-bucketed message log. Live tail (current bucket) isolates member writes from cold-history guest reads (older buckets). Hot path served by Redis hot-tail cache.';
