-- Wide-column flat-tree feed table. Eliminates ALLOW FILTERING for all list
-- operations by encoding the tree depth in the first clustering column:
--
--   parent_id = nil UUID (00000000-…) → top-level comment slot
--   parent_id = <comment_id>          → reply slot for that comment
--
-- Access patterns (all valid clustering-key prefix scans):
--   Top-level feed:   WHERE post_id = ? AND parent_id = <nil_uuid>   [AND created_at < ?] LIMIT ?
--   Reply thread:     WHERE post_id = ? AND parent_id = <comment_id>  [AND created_at < ?] LIMIT ?
--   Has-replies gate: SELECT comment_id FROM … WHERE post_id = ? AND parent_id = <comment_id> LIMIT 1
--
-- TWCS 7d is optimal here: comments are append-heavy, nearly immutable after
-- creation, and naturally time-bucketed. The 7-day window aligns with feed
-- retention expectations and keeps per-window SSTable counts low.
--
-- body and gif_* are denormalised from comment.comments to avoid N+1 fan-out
-- on list endpoints. On soft-delete both are nulled to reflect tombstone state.
CREATE TABLE IF NOT EXISTS comment.comments_by_post (
    post_id    uuid,
    parent_id  uuid,        -- nil UUID sentinel for top-level; comment_id for replies
    created_at timestamp,
    comment_id uuid,
    author_id  uuid,
    status     tinyint,
    body       text,
    gif_url    text,
    gif_width  int,
    gif_height int,
    PRIMARY KEY ((post_id), parent_id, created_at, comment_id)
) WITH CLUSTERING ORDER BY (parent_id ASC, created_at DESC, comment_id ASC)
  AND compaction  = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 7
  }
  AND compression = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Feed table for top-level and reply pagination. parent_id = nil UUID denotes top-level comments.';
