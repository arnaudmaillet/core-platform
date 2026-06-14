// crates/post/src/infrastructure/scylla/rows/scylla_media_row.rs

use crate::domain::entities::MediaAsset;
use crate::domain::types::{DurationSeconds, Height, MediaId, Width};
use crate::types::{MediaType, MimeType};
use infra_scylla::scylla_macros::{DeserializeValue, SerializeValue};
use shared_kernel::core::{Identifier, Result};
use shared_kernel::types::Url;
use uuid::Uuid;

#[derive(Debug, Clone, DeserializeValue, SerializeValue)]
#[scylla(crate = "infra_scylla::scylla")]
pub struct CqlMediaAssetRow {
    pub media_id: Uuid,
    pub url: String,
    pub thumbnail_url: String,
    pub duration_seconds: i32,
    pub width: i32,
    pub height: i32,
    pub media_type: String,
    pub mime_type: String,
}

impl CqlMediaAssetRow {
    pub fn from_domain(domain: &MediaAsset) -> Self {
        Self {
            media_id: domain.media_id().as_uuid(),
            url: domain.url().to_string(),
            thumbnail_url: domain.thumbnail_url().to_string(),
            duration_seconds: domain.duration_seconds().value() as i32,
            width: domain.width().value() as i32,
            height: domain.height().value() as i32,
            media_type: domain.media_type().to_string(),
            mime_type: domain.mime_type().to_string(),
        }
    }

    pub fn to_domain(self) -> Result<MediaAsset> {
        MediaAsset::builder(
            MediaId::new(self.media_id),
            Url::try_from(self.url)?,
            Url::try_from(self.thumbnail_url)?,
            DurationSeconds::try_new(self.duration_seconds as u32)?,
            Width::try_new(self.width as u32)?,
            Height::try_new(self.height as u32)?,
            MediaType::try_from(self.media_type)?,
            MimeType::try_from(self.mime_type)?,
        )
        .build()
    }
}
