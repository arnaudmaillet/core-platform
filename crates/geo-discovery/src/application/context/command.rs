// crates/geo_discovery/src/application/context/command.rs

use chrono::{DateTime, Utc};
use h3o::{LatLng, Resolution};
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use shared_kernel::core::{Error, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, ProfileId, Region};

use crate::context::GeoDiscoveryKernelCtx;
use crate::domain::types::{BucketHour, TileH3, TileResolution};
use crate::entities::MapAnnotation;
use crate::types::{PopularityScore, TilePostMetadata};

#[derive(Clone)]
pub struct GeoDiscoveryCommandCtx {
    kernel: GeoDiscoveryKernelCtx,
    operator_id: ProfileId,
    region_cmd: Region,
}

impl GeoDiscoveryCommandCtx {
    pub fn new(kernel: GeoDiscoveryKernelCtx, operator_id: ProfileId, region_cmd: Region) -> Self {
        Self {
            kernel,
            operator_id,
            region_cmd,
        }
    }

    pub fn app(&self) -> &GeoDiscoveryKernelCtx {
        &self.kernel
    }

    pub fn region(&self) -> Region {
        self.region_cmd
    }

    pub fn operator_id(&self) -> &ProfileId {
        &self.operator_id
    }

    pub fn verify_region(&self, command_region: Region) -> Result<()> {
        if command_region != self.region_cmd {
            return Err(Error::validation(
                "region",
                format!(
                    "Geo Sharding violation: Mismatch command region '{}' vs cluster sharding context '{}'",
                    command_region, self.region_cmd
                ),
            ));
        }
        Ok(())
    }

    pub async fn index_active_post(
        &self,
        metadata: TilePostMetadata,
        location: GeoPoint,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        popularity_score: PopularityScore,
        command_id: Option<Uuid>,
    ) -> Result<()> {
        if let Some(cmd_id) = command_id {
            if self.kernel.idempotency_repo().exists(None, &cmd_id).await? {
                return Err(Error::already_exists(
                    "GeoCommand",
                    "id",
                    cmd_id.to_string(),
                ));
            }
        }

        let persistence_repo = self.kernel.storage_repo();
        let cache_repo = self.kernel.cache_repo();

        let h3_lat_lng = LatLng::new(location.lat().to_radians(), location.lon().to_radians())
            .map_err(|e| {
                Error::validation("location", format!("Invalid coordinates for H3: {}", e))
            })?;

        let scylla_res = TileResolution::try_new(7)?;
        let scylla_cell = h3_lat_lng.to_cell(Resolution::try_from(7).unwrap());
        let scylla_tile = TileH3::from_str(&scylla_cell.to_string())?;

        let active_post_scylla =
            MapAnnotation::builder(metadata.post_id, location, scylla_res, scylla_tile)
                .with_post_type(metadata.post_type)
                .with_thumbnail_url(metadata.thumbnail_url.clone())
                .with_created_at(created_at)
                .with_expires_at(expires_at)
                .build()?;

        let ttl_duration = if active_post_scylla.expires_at() > active_post_scylla.created_at() {
            Duration::from_secs(
                (active_post_scylla.expires_at() - active_post_scylla.created_at()).num_seconds()
                    as u64,
            )
        } else {
            Duration::from_secs(0)
        };

        persistence_repo
            .save(&active_post_scylla, ttl_duration)
            .await?;

        // Multi-indexation Redis
        let target_resolutions = vec![3, 5, 7, 9, 10];
        for res_value in target_resolutions {
            let resolution = TileResolution::try_new(res_value)?;
            let h3_resolution = Resolution::try_from(res_value as u8).unwrap();
            let cell = h3_lat_lng.to_cell(h3_resolution);
            let tile_id = TileH3::from_str(&cell.to_string())?;

            cache_repo
                .add_to_tile(
                    resolution,
                    &tile_id,
                    &metadata,
                    popularity_score,
                    active_post_scylla.expires_at(),
                )
                .await?;

            cache_repo.track_active_tile(resolution, &tile_id).await?;
        }

        if let Some(cmd_id) = command_id {
            self.kernel.idempotency_repo().save(None, &cmd_id).await?;
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
            .kernel
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

        let persistence_repo = self.kernel.storage_repo();
        let cache_repo = self.kernel.cache_repo();

        let h3_lat_lng = LatLng::new(location.lat(), location.lon()).map_err(|e| {
            Error::validation(
                "location",
                format!(
                    "Coordonnées géographiques invalides pour la projection H3 : {}",
                    e
                ),
            )
        })?;

        // 1. Nettoyage ScyllaDB (Basé sur la rés. pivot 7)
        let scylla_cell = h3_lat_lng.to_cell(Resolution::try_from(7).unwrap());
        let scylla_tile = TileH3::from_str(&scylla_cell.to_string())?;
        let bucket = BucketHour::from_timestamp(created_at.timestamp_millis());

        persistence_repo
            .delete(TileResolution::try_new(7)?, &scylla_tile, bucket, post_id)
            .await?;

        // 2. Nettoyage chirurgical de tous les index de tuiles Redis via l'ID brut
        let target_resolutions = vec![3, 5, 7, 9, 10];
        for res_value in target_resolutions {
            let resolution = TileResolution::try_new(res_value)?;
            let h3_resolution = Resolution::try_from(res_value as u8).unwrap();
            let cell = h3_lat_lng.to_cell(h3_resolution);
            let tile_id = TileH3::from_str(&cell.to_string())?;

            cache_repo
                .remove_from_tile(resolution, &tile_id, post_id)
                .await?;
        }

        self.kernel
            .idempotency_repo()
            .save(None, &command_id)
            .await?;
        Ok(())
    }

    pub async fn execute_cache_eviction(
        &self,
        resolution: TileResolution,
        tile_id: &TileH3,
        older_than: DateTime<Utc>,
    ) -> Result<Vec<PostId>> {
        let evicted_metadata = self
            .kernel
            .cache_repo()
            .evict_old_posts(resolution, tile_id, older_than)
            .await?;

        Ok(evicted_metadata
            .into_iter()
            .map(|meta| meta.post_id)
            .collect())
    }
}
