CREATE KEYSPACE IF NOT EXISTS geo_discovery
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

CREATE TABLE IF NOT EXISTS geo_discovery.active_posts_by_tile (
    tile_resolution int,
    tile_id text,
    bucket_hour timestamp,
    post_id uuid,
    latitude double,
    longitude double,
    post_type text,
    thumbnail_url text,
    created_at timestamp,
    expires_at timestamp,
    PRIMARY KEY ((tile_resolution, tile_id, bucket_hour), post_id)
);