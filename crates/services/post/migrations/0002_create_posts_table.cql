CREATE TABLE IF NOT EXISTS post.posts (
    post_id      uuid,
    profile_id   uuid,
    kind         tinyint,
    status       tinyint,
    caption      text,
    attachments  text,
    parent_id    uuid,
    root_id      uuid,
    created_at   timestamp,
    updated_at   timestamp,
    published_at timestamp,
    deleted_at   timestamp,
    PRIMARY KEY (post_id)
) WITH compression = {'sstable_compression': 'LZ4Compressor'};
