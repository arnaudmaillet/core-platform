-- Approximate analytics counters for views, shares, and comments.
-- Redis is the authoritative real-time store; this table is populated by write-behind workers.
-- Counter tables are NOT idempotent on retry — accept approximate semantics for analytics.
CREATE TABLE IF NOT EXISTS engagement.post_interaction_counters (
    post_id       uuid PRIMARY KEY,
    view_count    counter,
    share_count   counter,
    comment_count counter
) WITH comment = 'Approximate interaction counters. Source of truth is Redis. Populated via write-behind flush and Kafka comment events.';
