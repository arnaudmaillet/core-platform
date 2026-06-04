// crates/geo_discovery/src/application/context/command.rs

use chrono::{DateTime, Utc};
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use shared_kernel::core::{Error, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, ProfileId, Region};

use crate::context::GeoDiscoveryAppContext;
use crate::domain::types::{BucketHour, H3Tile, TileResolution};
use crate::entities::ActiveMapPost;

#[derive(Clone)]
pub struct GeoDiscoveryCommandContext {
    app: GeoDiscoveryAppContext,
    operator_id: ProfileId,
    region: Region,
}

impl GeoDiscoveryCommandContext {
    pub fn new(app: GeoDiscoveryAppContext, operator_id: ProfileId, region: Region) -> Self {
        Self {
            app,
            operator_id,
            region,
        }
    }

    pub fn app(&self) -> &GeoDiscoveryAppContext {
        &self.app
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn ensure_executable(
        &self,
        command_id: Uuid,
        command_region: Region,
    ) -> Result<bool> {
        if command_region != self.region {
            return Err(Error::validation(
                "region",
                &format!(
                    "Geo Sharding violation: Mismatch '{}' vs '{}'",
                    command_region, self.region
                ),
            ));
        }
        let exists = self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?;
        Ok(!exists)
    }

    /// Centralise l'indexation : écrit 1 fois dans ScyllaDB (Pivot Rés. 7)
    /// et propage dans les 5 niveaux produits de Redis de manière transparente.
    pub async fn index_active_post(
        &self,
        post_id: PostId,
        location: GeoPoint,
        created_at: DateTime<Utc>,
        initial_score: f64,
        ttl: Duration,
        command_id: Option<Uuid>,
    ) -> Result<()> {
        if let Some(cmd_id) = command_id {
            if self.app.idempotency_repo().exists(None, &cmd_id).await? {
                return Err(Error::already_exists(
                    "GeoCommand",
                    "id",
                    cmd_id.to_string(),
                ));
            }
        }

        let persistence_repo = self.app.persistence_repo();
        let cache_repo = self.app.cache_repo();

        // 1. Projection H3 native
        let h3_lat_lng = LatLng::new(location.lat(), location.lon()).map_err(|e| {
            Error::validation("location", format!("Invalid coordinates for H3: {}", e))
        })?;

        // 2. Préparation et écriture ScyllaDB (Résolution Pivot 7)
        let scylla_res = TileResolution::try_new(7)?;
        let scylla_cell = h3_lat_lng.to_cell(Resolution::try_from(7).unwrap());
        let scylla_tile = H3Tile::from_str(&scylla_cell.to_string())?;

        let active_post_scylla = ActiveMapPost::builder(post_id, location, scylla_res, scylla_tile)
            .with_created_at(created_at)
            .build()?;

        // On lance la sauvegarde ScyllaDB
        persistence_repo.save(&active_post_scylla, ttl).await?;

        // 3. Multi-indexation Redis en arrière-plan
        let target_resolutions = vec![3, 5, 7, 9, 10];

        for res_value in target_resolutions {
            let resolution = TileResolution::try_new(res_value)?;
            let h3_resolution = Resolution::try_from(res_value as u8).unwrap();
            let cell = h3_lat_lng.to_cell(h3_resolution);
            let tile_id = H3Tile::from_str(&cell.to_string())?;

            // Alimentation synchrone ou concurrente de chaque niveau Redis
            cache_repo
                .add_to_tile(resolution, &tile_id, &post_id, initial_score, created_at)
                .await?;
        }

        if let Some(cmd_id) = command_id {
            self.app.idempotency_repo().save(None, &cmd_id).await?;
        }
        Ok(())
    }

    /// Supprime un post de TOUS les index Redis et de ScyllaDB de manière centralisée
    pub async fn remove_post_from_map(
        &self,
        location: GeoPoint,
        created_at: DateTime<Utc>,
        post_id: &PostId,
        command_id: Uuid,
    ) -> Result<()> {
        if self
            .app
            .idempotency_repo()
            .exists(None, &command_id)
            .await?
        {
            return Err(Error::already_exists(
                "GeoCommand",
                "id",
                command_id.to_string(),
            ));
        }

        let persistence_repo = self.app.persistence_repo();
        let cache_repo = self.app.cache_repo();

        let h3_lat_lng = LatLng::new(location.lat(), location.lon()).map_err(|e| {
            Error::validation(
                "location",
                &format!(
                    "Coordonnées géographiques invalides pour la projection H3 : {}",
                    e
                ),
            )
        })?;

        // 1. Nettoyage ScyllaDB (Basé sur la rés. pivot 7)
        let scylla_cell = h3_lat_lng.to_cell(Resolution::try_from(7).unwrap());
        let scylla_tile = H3Tile::from_str(&scylla_cell.to_string())?;
        let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

        persistence_repo
            .delete(TileResolution::try_new(7)?, &scylla_tile, bucket, post_id)
            .await?;

        // 2. Nettoyage de tous les niveaux de zoom Redis
        let target_resolutions = vec![3, 5, 7, 9, 10];
        for res_value in target_resolutions {
            let resolution = TileResolution::try_new(res_value)?;
            let h3_resolution = Resolution::try_from(res_value as u8).unwrap();
            let cell = h3_lat_lng.to_cell(h3_resolution);
            let tile_id = H3Tile::from_str(&cell.to_string())?;

            cache_repo
                .remove_from_tile(resolution, &tile_id, post_id)
                .await?;
        }

        self.app.idempotency_repo().save(None, &command_id).await?;
        Ok(())
    }

    pub async fn execute_cache_eviction(
        &self,
        resolution: TileResolution,
        tile_id: &H3Tile,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<PostId>> {
        self.app
            .cache_repo()
            .evict_old_posts(resolution, tile_id, older_than)
            .await
    }
}
