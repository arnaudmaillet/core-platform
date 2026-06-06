// crates/geo_discovery/src/application/commands/hydrate_tile.rs

use crate::domain::types::{TileH3, TileResolution};

#[derive(Debug, Clone)]
pub struct HydrateTileCacheCommand {
    pub resolution: TileResolution,
    pub tile_id: TileH3,
}

impl HydrateTileCacheCommand {
    pub fn new(resolution: TileResolution, tile_id: TileH3) -> Self {
        Self {
            resolution,
            tile_id,
        }
    }
}
