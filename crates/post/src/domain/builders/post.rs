// crates/post/src/domain/builders/post.rs

use chrono::{DateTime, Utc};
use shared_kernel::core::{LifecycleTracker, Result};
use shared_kernel::types::{MusicId, PostId, PostType, ProfileId};

use crate::entities::{MediaAsset, Post};
use crate::types::{Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel};

pub struct PostBuilder {
    post_id: PostId,
    author_id: ProfileId,
    post_type: PostType,
    visibility_level: VisibilityLevel,

    caption: Option<Caption>,
    media_list: Vec<MediaAsset>,
    music_id: Option<MusicId>,
    allowed_comment_hands: bool,
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
            visibility_level,
            caption: None,
            media_list: Vec::new(),
            music_id: None,
            allowed_comment_hands: true,
            hashtags: Hashtags::empty(),
            mentions: Mentions::empty(),
            edited_at: None,
            dynamic_metadata: None,
        }
    }

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

    pub fn with_edited_at(mut self, edited_at: Option<DateTime<Utc>>) -> Self {
        self.edited_at = edited_at;
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

        let extracted_hashtags = match (&self.caption, self.hashtags.is_empty()) {
            (Some(cap), true) => {
                Hashtags::try_from(cap.extract_hashtags().into_iter().collect::<Vec<String>>())?
            }
            (_, _) => self.hashtags,
        };

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
            extracted_hashtags,
            self.mentions,
            self.dynamic_metadata.unwrap_or_else(DynamicMetadata::empty),
            Utc::now(),
            self.edited_at,
            LifecycleTracker::default(),
            1,
        ))
    }
}
