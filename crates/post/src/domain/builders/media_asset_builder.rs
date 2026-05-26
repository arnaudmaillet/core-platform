// crates/post/src/domain/builders/media_asset.rs

use crate::domain::entities::MediaAsset;
use crate::domain::types::{DurationSeconds, Height, MediaId, MediaType, MimeType, Width};
use shared_kernel::core::{Result, ValueObject};
use shared_kernel::types::Url;

pub struct MediaAssetBuilder {
    media_id: MediaId,
    url: Url,
    thumbnail_url: Url,
    duration_seconds: DurationSeconds,
    width: Width,
    height: Height,
    media_type: MediaType,
    mime_type: MimeType,
}

impl MediaAssetBuilder {
    pub fn new(
        media_id: MediaId,
        url: Url,
        thumbnail_url: Url,
        duration: DurationSeconds,
        width: Width,
        height: Height,
        media_type: MediaType,
        mime_type: MimeType,
    ) -> Self {
        Self {
            media_id,
            url,
            thumbnail_url,
            duration_seconds: duration,
            width,
            height,
            media_type,
            mime_type,
        }
    }

    pub fn build(self) -> Result<MediaAsset> {
        let asset = MediaAsset::restore(
            self.media_id,
            self.url,
            self.thumbnail_url,
            self.duration_seconds,
            self.width,
            self.height,
            self.media_type,
            self.mime_type,
        );

        // Validation des invariants (ex: cohérence mime-type / type)
        asset.validate()?;

        Ok(asset)
    }
}
