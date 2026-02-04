-- crates/profile/migrations/scylla/202601030000_profile_stats.cql

CREATE TABLE IF NOT EXISTS profile_stats (
                                             account_id uuid,
                                             region_code text,
                                             follower_count counter,
                                             following_count counter,
                                             post_count counter,
                                             PRIMARY KEY (account_id, region_code)
);