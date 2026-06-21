-- Point-lookup index and deletion-key store for follow relationships.
--
-- Purpose 1 — Existence check: "Does profile A follow profile B?"
--   SELECT followed_at FROM social_graph.follow_status
--   WHERE follower_id = A AND followee_id = B;
--   O(1) by partition + full clustering key.
--
-- Purpose 2 — Deletion-key retrieval: ScyllaDB DELETE requires the full clustering key.
--   The followers and following tables include followed_at in their clustering key, so
--   unfollow and block-sever operations must know this timestamp before issuing DELETE.
--   This table is the canonical source for that value — no read-before-write on the
--   adjacency list tables is ever needed.
--
-- Partition key  : follower_id
-- Clustering key : followee_id  (single column; default ASC)
CREATE TABLE IF NOT EXISTS social_graph.follow_status (
    follower_id   uuid,
    followee_id   uuid,
    followed_at   timestamp,
    PRIMARY KEY (follower_id, followee_id)
) WITH compression = {'sstable_compression': 'LZ4Compressor'};
