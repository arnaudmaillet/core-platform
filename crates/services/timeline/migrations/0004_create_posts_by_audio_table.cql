CREATE TABLE IF NOT EXISTS timeline.posts_by_audio (
    audio_id     uuid,
    published_at timestamp,
    post_id      uuid,
    author_id    uuid,
    PRIMARY KEY ((audio_id), published_at, post_id)
) WITH CLUSTERING ORDER BY (published_at DESC, post_id ASC)
  AND compaction = {
      'class'                  : 'TimeWindowCompactionStrategy',
      'compaction_window_unit' : 'DAYS',
      'compaction_window_size' : 30
  }
  AND compression      = {'sstable_compression': 'LZ4Compressor'}
  AND gc_grace_seconds = 86400
  AND comment = 'Inverted index: all posts reusing a given audio track. Partitioned by audio_id. TWCS 30-day windows. No table-level TTL — rows purged via application DELETE on post.deleted. Hot path served by Redis audio:feed:{audio_id}.';
