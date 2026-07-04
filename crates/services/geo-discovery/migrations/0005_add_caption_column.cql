-- Online schema change: adds the caption column to map_post_cards.
--
-- Apply this migration against any existing deployment that was created from
-- migration 0003 before the caption (Focus-mode) field was introduced. New
-- deployments use the updated 0003 and do not need this migration.
--
-- ScyllaDB ALTER TABLE ADD is a metadata-only operation: existing rows are not
-- rewritten. Absent values read as NULL, which the application maps to an empty
-- caption — correct for any card indexed before post.published carried one.
--
-- caption is denormalized from post.published. It is served on the Focus-mode
-- read path (pin click / bottom-sheet expansion), never on the Radar pan path.
--
-- Scylla has no `ALTER TABLE ADD IF NOT EXISTS`, so on a fresh deployment (where
-- the updated 0003 already defines the column) this conflicts; the migration
-- runner treats an "already exists" ALTER as a benign no-op, so the full set
-- still applies idempotently to fresh and legacy clusters alike.
ALTER TABLE geo_discovery.map_post_cards
    ADD caption text;
