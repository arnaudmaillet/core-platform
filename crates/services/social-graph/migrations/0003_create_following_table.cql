-- Fan-out adjacency list: "Who does profile X follow?"
--
-- Symmetric denormalization of the followers table. The dual write (INSERT into both
-- followers and following per follow event) is the deliberate ScyllaDB trade-off that
-- eliminates ALLOW FILTERING on both read directions.
--
-- Partition key  : follower_id  — routes all follows of X to the same partition.
-- Clustering key : followed_at DESC, followee_id ASC
--   • Same rationale as followers: recent-first pagination with O(1) LIMIT.
--   • followee_id tie-breaks within the same millisecond.
CREATE TABLE IF NOT EXISTS social_graph.following (
    follower_id   uuid,
    followed_at   timestamp,
    followee_id   uuid,
    PRIMARY KEY (follower_id, followed_at, followee_id)
) WITH CLUSTERING ORDER BY (followed_at DESC, followee_id ASC)
  AND compression = {'sstable_compression': 'LZ4Compressor'};
