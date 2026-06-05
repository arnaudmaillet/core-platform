// crates/geo_discovery/src/domain/builders/active_map_post.rs

use crate::entities::ActiveMapPost;
use crate::types::{BucketHour, H3Tile, TileResolution};
use chrono::{DateTime, Duration, Utc};
use shared_kernel::core::{AggregateMetadata, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, PostType};

pub struct ActiveMapPostBuilder {
    post_id: PostId,
    location: GeoPoint,
    resolution: TileResolution,
    tile_id: H3Tile,
    post_type: Option<PostType>,
    thumbnail_url: Option<String>,
    created_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
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
            post_type: None,
            thumbnail_url: None,
            created_at: None,
            expires_at: None,
            metadata: AggregateMetadata::default(),
        }
    }

    pub fn with_post_type(mut self, post_type: PostType) -> Self {
        self.post_type = Some(post_type);
        self
    }

    pub fn with_thumbnail_url(mut self, thumbnail_url: Option<String>) -> Self {
        self.thumbnail_url = thumbnail_url;
        self
    }

    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn with_expires_at(mut self, expires_at: DateTime<Utc>) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn with_metadata(mut self, metadata: AggregateMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn build(self) -> Result<ActiveMapPost> {
        let created_at = self.created_at.unwrap_or_else(Utc::now);
        let bucket_hour = BucketHour::from_timestamp(created_at.timestamp_millis());
        let expires_at = self
            .expires_at
            .unwrap_or_else(|| created_at + Duration::hours(48));

        let post_type = self.post_type.unwrap_or(PostType::Text);

        Ok(ActiveMapPost::restore(
            self.post_id,
            self.location,
            self.resolution,
            self.tile_id,
            bucket_hour,
            post_type,
            self.thumbnail_url,
            created_at,
            expires_at,
            self.metadata,
        ))
    }
}
