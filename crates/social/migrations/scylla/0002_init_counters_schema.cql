-- crates/social/migrations/scylla/0002_init_counters_schema.cql

-- ============================================================================
-- TABLE 3 : Les Compteurs de profils (Profile Counters)
-- Stocke de manière résiliente et distribuée le volume de followers et followings.
-- ============================================================================
CREATE TABLE IF NOT EXISTS profile_counters (
    profile_id uuid,
    followers_count counter,
    following_count counter,
    PRIMARY KEY (profile_id)
)
WITH comment = 'Stockage persistant et atomique des compteurs consolides';