// crates/post/src/domain/entities/media_asset.rs

use serde::{Deserialize, Serialize};
use shared_kernel::core::{Entity, Error, Result, ValueObject};
use shared_kernel::types::Url;

use crate::domain::builders::MediaAssetBuilder;
use crate::domain::types::{DurationSeconds, Height, MediaId, MediaType, MimeType, Width};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaAsset {
    media_id: MediaId,
    url: Url,
    thumbnail_url: Url,
    duration_seconds: DurationSeconds,
    width: Width,
    height: Height,
    media_type: MediaType,
    mime_type: MimeType,
}

impl MediaAsset {
    pub fn builder(
        media_id: MediaId,
        url: Url,
        thumbnail_url: Url,
        duration: DurationSeconds,
        width: Width,
        height: Height,
        media_type: MediaType,
        mime_type: MimeType,
    ) -> MediaAssetBuilder {
        MediaAssetBuilder::new(
            media_id,
            url,
            thumbnail_url,
            duration,
            width,
            height,
            media_type,
            mime_type,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        media_id: MediaId,
        url: Url,
        thumbnail_url: Url,
        duration_seconds: DurationSeconds,
        width: Width,
        height: Height,
        media_type: MediaType,
        mime_type: MimeType,
    ) -> Self {
        Self {
            media_id,
            url,
            thumbnail_url,
            duration_seconds,
            width,
            height,
            media_type,
            mime_type,
        }
    }

    pub fn media_id(&self) -> MediaId {
        self.media_id
    }
    pub fn url(&self) -> &Url {
        &self.url
    }
    pub fn thumbnail_url(&self) -> &Url {
        &self.thumbnail_url
    }
    pub fn duration_seconds(&self) -> DurationSeconds {
        self.duration_seconds
    }
    pub fn width(&self) -> Width {
        self.width
    }
    pub fn height(&self) -> Height {
        self.height
    }
    pub fn media_type(&self) -> MediaType {
        self.media_type
    }
    pub fn mime_type(&self) -> &MimeType {
        &self.mime_type
    }
}

impl ValueObject for MediaAsset {
    fn validate(&self) -> Result<()> {
        if self.media_type.is_image() && self.duration_seconds.value() != 0 {
            return Err(Error::validation(
                "media_asset",
                "A image asset must have a duration of strictly 0 seconds",
            ));
        }

        if self.media_type.is_video() && self.duration_seconds.value() == 0 {
            return Err(Error::validation(
                "media_asset",
                "A video asset must have a duration greater than 0 seconds",
            ));
        }
        match self.media_type {
            MediaType::Video if !self.mime_type.is_video() => Err(Error::validation(
                "media_asset",
                format!(
                    "Media type is 'video' but MIME type '{}' is not a video format",
                    self.mime_type
                ),
            )),
            MediaType::Image if !self.mime_type.is_image() => Err(Error::validation(
                "media_asset",
                format!(
                    "Media type is 'image' but MIME type '{}' is not an image format",
                    self.mime_type
                ),
            )),
            _ => Ok(()),
        }
    }
}

impl Entity for MediaAsset {
    type Id = MediaId;

    fn id(&self) -> &Self::Id {
        &self.media_id
    }

    fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    fn entity_name() -> &'static str {
        "MediaAsset"
    }

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "media_id"
    }
}
