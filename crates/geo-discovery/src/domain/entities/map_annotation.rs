// crates/geo_discovery/src/domain/aggregates/active_map_post.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Entity, LifecycleTracker},
    geo::GeoPoint,
    messaging::{Event, EventEmitter},
    types::{PostId, PostType},
};

use crate::builders::MapAnnotationBuilder;
use crate::types::{BucketHour, TileH3, TileResolution};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapAnnotation {
    post_id: PostId,
    location: GeoPoint,
    resolution: TileResolution,
    tile_id: TileH3,
    bucket_hour: BucketHour,
    post_type: PostType,
    thumbnail_url: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    lifecycle: LifecycleTracker,
}

impl EventEmitter for MapAnnotation {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.lifecycle.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.lifecycle.pull_events()
    }
}

impl Entity for MapAnnotation {
    type Id = PostId;

    fn entity_name() -> &'static str {
        "ActiveMapPost"
    }

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "post_id"
    }

    fn id(&self) -> &Self::Id {
        self.post_id_as_ref()
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
}

impl MapAnnotation {
    pub fn builder(
        post_id: PostId,
        location: GeoPoint,
        resolution: TileResolution,
        tile_id: TileH3,
    ) -> MapAnnotationBuilder {
        MapAnnotationBuilder::new(post_id, location, resolution, tile_id)
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
        updated_at: DateTime<Utc>,
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
            lifecycle: LifecycleTracker::restore(updated_at),
        }
    }

    pub fn post_id(&self) -> PostId {
        self.post_id
    }

    pub(crate) fn post_id_as_ref(&self) -> &PostId {
        &self.post_id
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
