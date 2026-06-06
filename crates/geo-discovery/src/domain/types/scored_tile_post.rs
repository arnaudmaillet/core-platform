// crates/geo_discovery/src/domain/types/scored_post_tile.rs

use crate::domain::types::{PopularityScore, TilePostMetadata};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredPostTile {
    pub metadata: TilePostMetadata,
    pub popularity_score: PopularityScore,
}

impl ScoredPostTile {
    pub fn new(metadata: TilePostMetadata, popularity_score: PopularityScore) -> Self {
        Self {
            metadata,
            popularity_score,
        }
    }
}
