-- Migration 0002: conversation metadata (the Conversation aggregate state)
--
-- Partition key: conversation_id — one row per conversation, point-read on the
--   command path to hydrate/reconstitute the aggregate before a transition.
--
-- This table holds ONLY the bounded aggregate state. It never holds the audience:
--   subscribers/guests live in subscriptions_* and are never joined in here. This
--   is the physical expression of the aggregate consistency boundary — the reason
--   a viral public conversation does not drag millions of passive readers through
--   the member write/presence loops.
--
-- Mutable columns (UPDATEd in place):
--   visibility, public_since  → the Private/Public toggle + watermark
--   member_count, updated_at  → roster growth/shrink
-- Immutable after creation: kind (topology), owner_id, created_at.
--
-- public_since is the watermark MessageId (UUID v7). NULL while private; once
--   public, the audience may read only messages at/after this boundary.
--
-- Compaction: LeveledCompactionStrategy. This is a read-mostly, point-lookup
--   table with occasional small UPDATEs (toggle, counter) — LCS keeps read
--   amplification at ~1 SSTable per lookup, which STCS/TWCS would not.
CREATE TABLE IF NOT EXISTS chat.conversations (
    conversation_id uuid,
    kind            tinyint,    -- 0=Group, 1=Channel (immutable topology)
    visibility      tinyint,    -- 0=Private, 1=Public (mutable toggle)
    owner_id        uuid,
    member_count    int,        -- bounded Member Plane size (<= 500 for Group)
    public_since    uuid,       -- watermark MessageId (UUID v7); NULL while private
    created_at      timestamp,
    updated_at      timestamp,
    PRIMARY KEY ((conversation_id))
) WITH compaction  = {'class': 'LeveledCompactionStrategy'}
  AND compression  = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Conversation aggregate state. Point-read on the command path. Audience never stored here.';
