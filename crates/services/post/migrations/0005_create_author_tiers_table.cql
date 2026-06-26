-- Denormalized author tier projection, maintained from profile.v1.events
-- (ProfileTierChanged). Read on the publish path to stamp `author_tier` onto
-- post.published / post.v1.events so timeline routes VIP authors to its read path.
-- 0=Standard, 1=Premium, 2=Vip.
CREATE TABLE IF NOT EXISTS post.author_tiers (
    profile_id uuid PRIMARY KEY,
    tier       tinyint
) WITH compression = {'sstable_compression': 'LZ4Compressor'};
