CREATE KEYSPACE IF NOT EXISTS geo_discovery 
WITH replication = {'class': 'NetworkTopologyStrategy', 'replication_factor': 3};

CREATE TABLE geo_discovery.active_posts_by_tile (
    tile_resolution int,
    tile_id text,
    bucket_hour timestamp,
    post_id uuid,
    latitude double,
    longitude double,
    created_at timestamp,
    PRIMARY KEY ((tile_resolution, tile_id, bucket_hour), post_id)
);