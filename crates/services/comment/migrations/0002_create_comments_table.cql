-- Source-of-truth store for single comment point reads and lifecycle mutations.
-- Partitioned by comment_id for O(1) lookup without knowing the post partition.
-- body and gif_* columns are nullable: gif-only comments have no body, text-only
-- have no gif, and tombstoned comments have both nulled on soft-delete.
-- LCS minimises read amplification for the mixed point-read / status-update workload.
CREATE TABLE IF NOT EXISTS comment.comments (
    comment_id uuid,
    post_id    uuid,
    author_id  uuid,
    parent_id  uuid,        -- nil UUID (00000000-…) for top-level comments
    status     tinyint,     -- 0 = Published, 1 = Deleted
    body       text,
    gif_id     text,
    gif_url    text,
    gif_width  int,
    gif_height int,
    created_at timestamp,
    updated_at timestamp,
    deleted_at timestamp,
    PRIMARY KEY (comment_id)
) WITH compaction  = {'class': 'LeveledCompactionStrategy'}
  AND compression  = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Source-of-truth comment store. Used for point reads and lifecycle mutations (soft-delete / purge).';
