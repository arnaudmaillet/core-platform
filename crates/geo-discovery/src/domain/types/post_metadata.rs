// crates/geo_discovery/src/domain/types/tile_post_metadata.rs

use serde::{Deserialize, Serialize};
use shared_kernel::types::{PostId, PostType};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, PartialOrd)]
pub struct TilePostMetadata {
    pub post_id: PostId,
    pub latitude: f64,
    pub longitude: f64,
    pub post_type: PostType,
    pub thumbnail_url: Option<String>,
}

impl TilePostMetadata {
    pub fn new(
        post_id: PostId,
        latitude: f64,
        longitude: f64,
        post_type: PostType,
        thumbnail_url: Option<String>,
    ) -> Self {
        Self {
            post_id,
            latitude,
            longitude,
            post_type,
            thumbnail_url,
        }
    }
}
