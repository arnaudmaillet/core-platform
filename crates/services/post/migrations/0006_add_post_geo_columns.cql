-- Online schema change: adds client-supplied location columns to post.posts.
--
-- Coordinates are captured at CreatePost and replayed at publish so that
-- post.published can carry lat/lng for geo-discovery's spatial index. Posts
-- without a location leave these NULL and are simply not geo-indexed.
--
-- ScyllaDB ALTER TABLE ADD is a metadata-only operation: existing rows are not
-- rewritten. Absent values read as NULL → the aggregate reconstitutes
-- `location = None`, which is correct for any pre-existing locationless post.
--
-- Scylla has no `ALTER TABLE ADD IF NOT EXISTS`; the migration runner treats an
-- "already exists" ALTER as a benign no-op, so this applies idempotently to
-- fresh and legacy clusters alike.
ALTER TABLE post.posts ADD lat double;
ALTER TABLE post.posts ADD lng double;
