-- Online schema change: adds the author_tier column to map_post_cards.
--
-- Apply this migration against any existing deployment that was created from
-- migration 0003 before the author_tier feature was introduced. New deployments
-- use the updated 0003 and do not need this migration.
--
-- ScyllaDB ALTER TABLE ADD is a metadata-only operation: existing rows are not
-- rewritten. Absent values read as NULL, which the application maps to
-- AuthorTier::Standard (0) — the safe default for ungated legacy posts.
--
-- After applying, TierSyncWorker will backfill rows as profile.tier_changed
-- events arrive. There is no need for a bulk backfill job; Standard is the
-- correct tier for any post whose author has never changed tier.
ALTER TABLE geo_discovery.map_post_cards
    ADD author_tier tinyint;
