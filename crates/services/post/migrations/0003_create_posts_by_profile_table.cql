CREATE TABLE IF NOT EXISTS post.posts_by_profile (
    profile_id  uuid,
    created_at  timestamp,
    post_id     uuid,
    kind        tinyint,
    status      tinyint,
    PRIMARY KEY (profile_id, created_at, post_id)
) WITH CLUSTERING ORDER BY (created_at DESC, post_id ASC)
  AND compression = {'sstable_compression': 'LZ4Compressor'};
