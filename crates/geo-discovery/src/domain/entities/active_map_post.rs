// crates/geo_discovery/src/domain/aggregates/active_map_post.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{AggregateMetadata, AggregateRoot, Versioned},
    geo::GeoPoint,
    messaging::{Event, EventEmitter},
    types::{PostId, PostType},
};

use crate::{
    builders::ActiveMapPostBuilder,
    domain::types::{BucketHour, TileH3, TileResolution},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMapPost {
    post_id: PostId,
    location: GeoPoint,
    resolution: TileResolution,
    tile_id: TileH3,
    bucket_hour: BucketHour,
    post_type: PostType,
    thumbnail_url: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl Versioned for ActiveMapPost {
    fn version(&self) -> u64 {
        self.metadata.version()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
    fn record_change(&mut self) {
        self.metadata.record_change();
    }
}

impl EventEmitter for ActiveMapPost {
    fn push_event(&mut self, _event: Box<dyn Event>) {
        self.metadata.push_event(_event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl AggregateRoot for ActiveMapPost {
    fn id(&self) -> String {
        self.post_id.to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}

impl ActiveMapPost {
    pub fn builder(
        post_id: PostId,
        location: GeoPoint,
        resolution: TileResolution,
        tile_id: TileH3,
    ) -> ActiveMapPostBuilder {
        ActiveMapPostBuilder::new(post_id, location, resolution, tile_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        post_id: PostId,
        location: GeoPoint,
        resolution: TileResolution,
        tile_id: TileH3,
        bucket_hour: BucketHour,
        post_type: PostType,
        thumbnail_url: Option<String>,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            post_id,
            location,
            resolution,
            tile_id,
            bucket_hour,
            post_type,
            thumbnail_url,
            created_at,
            expires_at,
            metadata,
        }
    }

    pub fn post_id(&self) -> PostId {
        self.post_id
    }
    pub fn location(&self) -> GeoPoint {
        self.location
    }
    pub fn resolution(&self) -> TileResolution {
        self.resolution
    }
    pub fn tile_id(&self) -> &TileH3 {
        &self.tile_id
    }
    pub fn bucket_hour(&self) -> BucketHour {
        self.bucket_hour
    }
    pub fn post_type(&self) -> PostType {
        self.post_type
    }
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.thumbnail_url.as_deref()
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }
}
