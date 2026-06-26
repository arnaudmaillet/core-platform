-- media metadata System of Record (Postgres).
--
-- The byte *content* lives in object storage; this table holds the canonical
-- *truth about* each asset: its lifecycle state, owner, verified facts, and its
-- rendition catalog. The rich aggregate (including renditions) is stored as a
-- JSONB document so the schema tracks the domain without a wide column list or a
-- separate renditions table; the few columns that must be queried or indexed
-- (owner, state, content hash) are projected out alongside.

CREATE TABLE IF NOT EXISTS assets (
    id            UUID         PRIMARY KEY,
    owner_id      UUID         NOT NULL,
    kind          TEXT         NOT NULL,
    state         TEXT         NOT NULL,
    -- Lowercase-hex SHA-256 of the original bytes; NULL until finalized. Drives
    -- content-addressing and (when enabled) cross-owner dedup.
    content_hash  TEXT,
    created_at    TIMESTAMPTZ  NOT NULL,
    updated_at    TIMESTAMPTZ  NOT NULL,
    -- The full Asset aggregate (serde JSON). Source of truth for reconstruction.
    doc           JSONB        NOT NULL
);

-- Owner lookup (orphan GC, per-owner listing).
CREATE INDEX IF NOT EXISTS idx_assets_owner ON assets (owner_id);

-- Dedup lookup: find a READY asset with these exact bytes. Partial index keeps it
-- small (only finalized assets carry a hash).
CREATE INDEX IF NOT EXISTS idx_assets_content_hash
    ON assets (content_hash)
    WHERE content_hash IS NOT NULL;
