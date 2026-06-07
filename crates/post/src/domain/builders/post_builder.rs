// crates/post/src/domain/builders/post.rs

use chrono::{DateTime, Utc};
use shared_kernel::core::{LifecycleTracker, Result};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};

use crate::domain::entities::MediaAsset;
use crate::domain::entities::Post;
use crate::domain::types::{Caption, DynamicMetadata, Hashtags, VisibilityLevel};
use crate::types::Mentions;

pub struct PostBuilder {
    post_id: PostId,
    author_id: ProfileId,
    post_type: PostType,
    caption: Option<Caption>,
    visibility_level: VisibilityLevel,
    media_list: Vec<MediaAsset>,
    music_id: Option<MusicId>,
    allowed_comment_hands: bool,
    is_edited: bool,
    hashtags: Hashtags,
    mentions: Mentions,
    edited_at: Option<DateTime<Utc>>,
    dynamic_metadata: Option<DynamicMetadata>,
}

impl PostBuilder {
    pub(crate) fn new(
        post_id: PostId,
        author_id: ProfileId,
        post_type: PostType,
        visibility_level: VisibilityLevel,
    ) -> Self {
        Self {
            post_id,
            author_id,
            post_type,
            caption: None,
            visibility_level,
            media_list: Vec::new(),
            music_id: None,
            allowed_comment_hands: true,
            is_edited: false,
            hashtags: Hashtags::empty(),
            mentions: Mentions::empty(),
            edited_at: None,
            dynamic_metadata: None,
        }
    }

    // --- SETTERS POUR CHAMPS OPTIONNELS ---

    pub fn with_media_list(mut self, media: Vec<MediaAsset>) -> Self {
        self.media_list = media;
        self
    }

    pub fn with_caption(mut self, caption: Caption) -> Self {
        self.caption = Some(caption);
        self
    }

    pub fn with_optional_caption(mut self, caption: Option<Caption>) -> Self {
        self.caption = caption;
        self
    }

    pub fn with_music_id(mut self, music_id: MusicId) -> Self {
        self.music_id = Some(music_id);
        self
    }

    pub fn with_optional_music_id(mut self, music_id: Option<MusicId>) -> Self {
        self.music_id = music_id;
        self
    }

    pub fn with_comment_settings(mut self, allowed: bool) -> Self {
        self.allowed_comment_hands = allowed;
        self
    }

    pub fn with_edit_status(mut self, is_edited: bool, updated_at: Option<DateTime<Utc>>) -> Self {
        self.is_edited = is_edited;
        self.edited_at = updated_at;
        self
    }

    pub fn with_dynamic_metadata(mut self, metadata: DynamicMetadata) -> Self {
        self.dynamic_metadata = Some(metadata);
        self
    }

    pub fn with_mentions(mut self, mentions: Mentions) -> Self {
        self.mentions = mentions;
        self
    }

    pub fn with_hashtags(mut self, hashtags: Hashtags) -> Self {
        self.hashtags = hashtags;
        self
    }

    pub fn build(self) -> Result<Post> {
        Post::validate_invariants(self.post_type, &self.media_list, &self.caption)?;
        let total_duration: u32 = self
            .media_list
            .iter()
            .map(|m| m.duration_seconds().value())
            .sum();

        Ok(Post::restore(
            self.post_id,
            self.author_id,
            self.post_type,
            self.caption,
            self.media_list,
            total_duration,
            self.allowed_comment_hands,
            self.visibility_level,
            self.music_id,
            self.hashtags,
            self.mentions,
            self.dynamic_metadata.unwrap_or_else(DynamicMetadata::empty),
            self.edited_at,
            LifecycleTracker::default(),
        ))
    }
}
