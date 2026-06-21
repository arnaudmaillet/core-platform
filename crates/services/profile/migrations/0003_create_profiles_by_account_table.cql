CREATE TABLE IF NOT EXISTS profile.profiles_by_account (
    account_id    uuid,
    created_at    timestamp,
    profile_id    uuid,
    handle        text,
    display_name  text,
    avatar_url    text,
    profile_kind  text,
    visibility    text,
    status        text,
    PRIMARY KEY (account_id, created_at, profile_id)
) WITH CLUSTERING ORDER BY (created_at DESC, profile_id ASC)
  AND compression = {'sstable_compression': 'LZ4Compressor'};
