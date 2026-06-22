-- Migration 003: per-profile unread notification counter
--
-- ScyllaDB counter table providing durable unread badge counts.
-- Redis key `notification:unread:{profile_id}` acts as the L1 hot-path cache.
--
-- Write protocol:
--   New notification  → UPDATE … SET unread_count = unread_count + 1
--                     + INCR in Redis (no TTL)
--   Mark single read  → UPDATE … SET unread_count = unread_count - 1
--                     + DECR in Redis
--   Mark all as read  → DELETE this row (resets to 0 on next increment)
--                     + SET notification:unread:{id} 0 in Redis
--                     + SET notification:read_horizon:{id} {now_ms} in Redis
--
-- Reads: Redis first; on cache miss fall back to SELECT here.
--
-- Counter tables in ScyllaDB use a dedicated replica-level merge strategy
-- (LWW per-shard accumulator). Do NOT mix counter and non-counter columns
-- in the same table.

CREATE TABLE IF NOT EXISTS notification.notification_unread_counters (
    target_profile_id uuid PRIMARY KEY,
    unread_count      counter
)
WITH comment = 'Durable unread badge count per profile. Redis is the L1 cache.';
