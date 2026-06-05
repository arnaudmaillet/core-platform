// crates/geo_discovery/src/application/commands/hydrate_tile.rs

use crate::domain::types::{H3Tile, TileResolution};

#[derive(Debug, Clone)]
pub struct HydrateTileCacheCommand {
    pub resolution: TileResolution,
    pub tile_id: H3Tile,
}

impl HydrateTileCacheCommand {
    pub fn new(resolution: TileResolution, tile_id: H3Tile) -> Self {
        Self {
            resolution,
            tile_id,
        }
    }
}
