// crates/geo_discovery/src/domain/builders/active_map_post.rs

use chrono::{DateTime, Utc};
use shared_kernel::core::{AggregateMetadata, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::PostId;

use crate::entities::ActiveMapPost;
use crate::types::{BucketHour, H3Tile, TileResolution};

pub struct ActiveMapPostBuilder {
    post_id: PostId,
    location: GeoPoint,
    resolution: TileResolution,
    tile_id: H3Tile,
    created_at: Option<DateTime<Utc>>,
    metadata: AggregateMetadata,
}

impl ActiveMapPostBuilder {
    pub fn new(
        post_id: PostId,
        location: GeoPoint,
        resolution: TileResolution,
        tile_id: H3Tile,
    ) -> Self {
        Self {
            post_id,
            location,
            resolution,
            tile_id,
            created_at: None,
            metadata: AggregateMetadata::default(),
        }
    }

    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn with_metadata(mut self, metadata: AggregateMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn build(self) -> Result<ActiveMapPost> {
        let created_at = self.created_at.unwrap_or_else(Utc::now);
        let bucket_hour = BucketHour::from_timestamp(created_at.timestamp_millis());

        Ok(ActiveMapPost::restore(
            self.post_id,
            self.location,
            self.resolution,
            self.tile_id,
            bucket_hour,
            created_at,
            self.metadata,
        ))
    }
}
