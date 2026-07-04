-- Block adjacency list and point-lookup table.
--
-- A single table serves three access patterns without a mirror:
--   1. "Has A blocked B?"  — SELECT WHERE blocker_id = A AND blockee_id = B  (O(1))
--   2. "Has B blocked A?"  — SELECT WHERE blocker_id = B AND blockee_id = A  (O(1))
--   3. "List all profiles A has blocked" — full partition scan on blocker_id = A
--
-- Pattern 1 and 2 are the block-gate checks performed during FollowProfile commands.
-- Both are O(1) point lookups on the same table with swapped arguments — no mirror
-- (blocked_by) table is needed.
--
-- "Who blocked me?" is intentionally not exposed to end-users; the service only
-- uses the two-direction gate check (patterns 1+2) for enforcement.
--
-- Partition key  : blocker_id
-- Clustering key : blockee_id  (default ASC — paginates block lists in UUID order)
CREATE TABLE IF NOT EXISTS social_graph.blocks (
    blocker_id    uuid,
    blockee_id    uuid,
    blocked_at    timestamp,
    PRIMARY KEY (blocker_id, blockee_id)
) WITH compression = {'sstable_compression': 'LZ4Compressor'};
