// crates/post/src/application/commands/create_post.rs

use std::str::FromStr;

use serde::Deserialize;
use shared_kernel::command::{CommandTarget, IdentifiableCommand};
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId, Region, Url};
use shared_proto::post::v1::CreatePostRequest;
use shared_proto::post::v1::MediaAsset as ProtoMediaAsset;
use uuid::Uuid;

use crate::entities::MediaAsset;

use crate::types::{
    Caption, DurationSeconds, DynamicMetadata, Height, MediaId, MediaType, MimeType, Width,
};

#[derive(Debug, Deserialize, Clone)]
pub struct CreatePostCommand {
    pub command_id: Uuid,
    pub target: CommandTarget<ProfileId>,
    pub region: Region,
    pub post_id: PostId,
    pub post_type: PostType,
    pub caption: Option<Caption>,
    pub media_list: Vec<MediaAsset>,
    pub allowed_comment_hands: bool,
    pub visibility_level: String,
    pub music_id: Option<MusicId>,
    pub dynamic_metadata: Option<DynamicMetadata>,
}

impl IdentifiableCommand for CreatePostCommand {
    type Id = ProfileId;
    type Routing = Region;

    fn command_id(&self) -> Uuid {
        self.command_id
    }

    fn target(&self) -> &CommandTarget<ProfileId> {
        &self.target
    }

    fn routing(&self) -> Self::Routing {
        self.region
    }

    fn resolve_cache_key(&self) -> Option<String> {
        None
    }
}

impl CreatePostCommand {
    pub fn try_from_proto(req: CreatePostRequest, post_id: PostId) -> Result<Self> {
        let command_id = Uuid::parse_str(&req.command_id)
            .map_err(|_| Error::validation("command_id", "Invalid UUID"))?;
        let region = Region::try_new(req.region)
            .map_err(|_| Error::validation("region", "Invalid region"))?;

        let author_id = ProfileId::try_new(req.author_id)?;
        let target = CommandTarget::stateless(author_id);
        let post_type = PostType::try_from(req.post_type)?;
        let caption = match req.caption {
            Some(text) if !text.trim().is_empty() => Some(Caption::try_new(text)?),
            _ => None,
        };

        let dynamic_metadata = if req.dynamic_metadata.trim().is_empty() {
            None
        } else {
            let json_val: serde_json::Value = serde_json::from_str(&req.dynamic_metadata)
                .map_err(|_| Error::validation("dynamic_metadata", "Invalid JSON"))?;
            Some(DynamicMetadata::try_new(json_val)?)
        };
        let music_id = req.music_id.map(|s| MusicId::from_str(&s)).transpose()?;

        let media_list = req
            .media_list
            .into_iter()
            .map(|m| MediaAsset::from_proto(m))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            command_id,
            post_id,
            target,
            region,
            post_type,
            caption,
            media_list,
            allowed_comment_hands: req.allowed_comment_hands,
            visibility_level: req.visibility_level,
            music_id,
            dynamic_metadata,
        })
    }
}

impl MediaAsset {
    pub fn from_proto(proto: ProtoMediaAsset) -> Result<Self> {
        Ok(Self::restore(
            MediaId::try_from(proto.media_id)?,
            Url::try_from(proto.url)?,
            Url::try_from(proto.thumbnail_url)?,
            DurationSeconds::try_new(proto.duration_seconds)?,
            Width::try_new(proto.width)?,
            Height::try_new(proto.height)?,
            MediaType::from_str(&proto.media_type)?,
            MimeType::from_str(&proto.mime_type)?,
        ))
    }
}
