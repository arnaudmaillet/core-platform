// crates/geo_discovery/src/application/context/query.rs

use shared_kernel::core::Result;
use shared_kernel::types::{PostId, Region};

use crate::context::GeoDiscoveryAppContext;
use crate::types::{H3Tile, TileResolution};

#[derive(Clone)]
pub struct GeoDiscoveryQueryContext {
    app: GeoDiscoveryAppContext,
    region: Region,
}

impl GeoDiscoveryQueryContext {
    pub fn new(app: GeoDiscoveryAppContext, region: Region) -> Self {
        Self { app, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn get_top_posts_in_tile(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        limit: usize,
    ) -> Result<Vec<PostId>> {
        self.app
            .cache_repo()
            .get_top_posts(resolution, tile_id, limit)
            .await
    }
}
