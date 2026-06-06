// crates/geo_discovery/src/infrastructure/mappers/cql_map_post.rs

use chrono::{TimeZone, Utc};
use infra_scylla::scylla::value::CqlTimestamp;
use infra_scylla::scylla::{self, DeserializeRow};
use shared_kernel::core::{AggregateMetadata, Error, Identifier, Result};
use shared_kernel::geo::GeoPoint;
use shared_kernel::types::{PostId, PostType};
use std::str::FromStr;
use uuid::Uuid;

use crate::entities::ActiveMapPost;
use crate::types::{BucketHour, TileH3, TileResolution};

#[derive(Debug, DeserializeRow)]
pub struct CqlMapPostRow {
    pub tile_resolution: i32,
    pub tile_id: String,
    pub bucket_hour: CqlTimestamp,
    pub post_id: Uuid,
    pub latitude: f64,
    pub longitude: f64,
    pub post_type: String,
    pub thumbnail_url: Option<String>,
    pub created_at: CqlTimestamp,
    pub expires_at: CqlTimestamp,
}

impl TryFrom<CqlMapPostRow> for ActiveMapPost {
    type Error = Error;

    fn try_from(row: CqlMapPostRow) -> Result<Self> {
        let post_id = PostId::from_uuid(row.post_id);
        let location = GeoPoint::try_new(row.longitude, row.latitude)?;
        let resolution = TileResolution::try_new(row.tile_resolution)?;
        let tile_id = TileH3::from_str(&row.tile_id)?;
        let bucket_hour = BucketHour::from_timestamp(row.bucket_hour.0);

        let post_type = PostType::from_str(&row.post_type)?;
        let thumbnail_url = row.thumbnail_url.filter(|url| !url.is_empty());

        let created_at = Utc
            .timestamp_millis_opt(row.created_at.0)
            .single()
            .ok_or_else(|| {
                Error::validation("created_at", "Invalid created_at timestamp format")
            })?;

        let expires_at = Utc
            .timestamp_millis_opt(row.expires_at.0)
            .single()
            .ok_or_else(|| {
                Error::validation("expires_at", "Invalid expires_at timestamp format")
            })?;

        Ok(ActiveMapPost::restore(
            post_id,
            location,
            resolution,
            tile_id,
            bucket_hour,
            post_type,
            thumbnail_url,
            created_at,
            expires_at,
            AggregateMetadata::default(),
        ))
    }
}
