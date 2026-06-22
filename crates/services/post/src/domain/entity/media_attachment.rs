use serde::{Deserialize, Serialize};
use crate::domain::value_object::{CdnUrl, MimeType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    pub cdn_url:          CdnUrl,
    pub mime_type:        MimeType,
    pub width:            u32,
    pub height:           u32,
    pub thumbnail_url:    Option<CdnUrl>,
    pub duration_seconds: Option<f32>,
}

impl MediaAttachment {
    pub fn is_video(&self) -> bool {
        self.mime_type.is_video()
    }
}
