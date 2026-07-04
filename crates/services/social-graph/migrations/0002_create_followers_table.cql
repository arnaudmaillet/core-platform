-- Fan-in adjacency list: "Who follows profile X?"
--
-- Partition key  : followee_id  — routes all followers of X to the same ScyllaDB partition.
-- Clustering key : followed_at DESC, follower_id ASC
--   • DESC on followed_at ensures a LIMIT N scan always returns the N most-recent followers
--     without a coordinator-side sort.
--   • follower_id provides a deterministic tie-breaker when two followers arrive within the
--     same millisecond (prevents silent row collisions).
--
-- DELETE requirement: callers must supply the full clustering key (followed_at + follower_id).
-- The follow_status table is the canonical store for followed_at — see migration 0004.
CREATE TABLE IF NOT EXISTS social_graph.followers (
    followee_id   uuid,
    followed_at   timestamp,
    follower_id   uuid,
    PRIMARY KEY (followee_id, followed_at, follower_id)
) WITH CLUSTERING ORDER BY (followed_at DESC, follower_id ASC)
  AND compression = {'sstable_compression': 'LZ4Compressor'};
