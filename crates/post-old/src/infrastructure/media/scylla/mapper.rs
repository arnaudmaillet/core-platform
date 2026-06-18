// crates/post/src/infrastructure/media/scylla/mapper.rs

use crate::domain::entities::MediaAsset;
use crate::domain::types::{DurationSeconds, Height, MediaId, Width};
use crate::infrastructure::media::ScyllaMediaModel;
use crate::types::{MediaType, MimeType};

use shared_kernel::core::{Identifier, Result};
use shared_kernel::types::Url;

/// Convertit le MediaAsset du Domaine en Row Scylla
impl From<&MediaAsset> for ScyllaMediaModel {
    fn from(domain: &MediaAsset) -> Self {
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
}

/// Convertit la Row Scylla en MediaAsset du Domaine
impl TryFrom<ScyllaMediaModel> for MediaAsset {
    type Error = shared_kernel::core::Error;

    fn try_from(row: ScyllaMediaModel) -> Result<Self> {
        MediaAsset::builder(
            MediaId::new(row.media_id),
            Url::try_from(row.url)?,
            Url::try_from(row.thumbnail_url)?,
            DurationSeconds::try_new(row.duration_seconds as u32)?,
            Width::try_new(row.width as u32)?,
            Height::try_new(row.height as u32)?,
            MediaType::try_from(row.media_type)?,
            MimeType::try_from(row.mime_type)?,
        )
        .build()
    }
}
