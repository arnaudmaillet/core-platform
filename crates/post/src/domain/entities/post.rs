// crates/post/src/domain/aggregates/post.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{AggregateMetadata, AggregateRoot, Error, Result, Versioned},
    messaging::{Event, EventEmitter, OperationTracker},
    types::{MusicId, PostId, ProfileId},
};

use crate::{
    domain::{
        builders::PostBuilder,
        entities::MediaAsset,
        events::PostEvent,
        types::{Caption, DynamicMetadata, Hashtags, PostType, VisibilityLevel},
    },
    types::Mentions,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    post_id: PostId,
    author_id: ProfileId,
    post_type: PostType,
    caption: Option<Caption>,
    media_list: Vec<MediaAsset>,
    total_duration_seconds: u32,
    allowed_comment_hands: bool,
    visibility_level: VisibilityLevel,
    music_id: Option<MusicId>,
    hashtags: Hashtags,
    mentions: Mentions,
    is_edited: bool,
    updated_at: Option<DateTime<Utc>>,
    dynamic_metadata: DynamicMetadata,
    metadata: AggregateMetadata,
}

impl Versioned for Post {
    fn version(&self) -> u64 {
        self.metadata.version()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
    fn record_change(&mut self) {
        self.metadata.record_change();
    }
}

impl EventEmitter for Post {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl AggregateRoot for Post {
    fn id(&self) -> String {
        self.post_id.to_string()
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}

impl Post {
    pub fn builder(
        post_id: PostId,
        author_id: ProfileId,
        post_type: PostType,
        visibility: VisibilityLevel,
    ) -> PostBuilder {
        PostBuilder::new(post_id, author_id, post_type, visibility)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        post_id: PostId,
        author_id: ProfileId,
        post_type: PostType,
        caption: Option<Caption>,
        media_list: Vec<MediaAsset>,
        total_duration_seconds: u32,
        allowed_comment_hands: bool,
        visibility_level: VisibilityLevel,
        music_id: Option<MusicId>,
        hashtags: Hashtags,
        mentions: Mentions,
        is_edited: bool,
        updated_at: Option<DateTime<Utc>>,
        dynamic_metadata: DynamicMetadata,
        metadata: AggregateMetadata,
    ) -> Self {
        Self {
            post_id,
            author_id,
            post_type,
            caption,
            media_list,
            total_duration_seconds,
            allowed_comment_hands,
            visibility_level,
            music_id,
            hashtags,
            mentions,
            is_edited,
            updated_at,
            dynamic_metadata,
            metadata,
        }
    }

    // --- GETTERS ---
    pub fn post_id(&self) -> PostId {
        self.post_id
    }
    pub fn author_id(&self) -> ProfileId {
        self.author_id
    }
    pub fn post_type(&self) -> PostType {
        self.post_type
    }
    pub fn caption(&self) -> &Option<Caption> {
        &self.caption
    }
    pub fn media_list(&self) -> &[MediaAsset] {
        &self.media_list
    }
    pub fn total_duration_seconds(&self) -> u32 {
        self.total_duration_seconds
    }
    pub fn allowed_comment_hands(&self) -> bool {
        self.allowed_comment_hands
    }
    pub fn visibility_level(&self) -> VisibilityLevel {
        self.visibility_level
    }
    pub fn music_id(&self) -> Option<MusicId> {
        self.music_id
    }
    pub fn hashtags(&self) -> &Hashtags {
        &self.hashtags
    }
    pub fn mentions(&self) -> &Mentions {
        &self.mentions
    }
    pub fn is_edited(&self) -> bool {
        self.is_edited
    }
    pub fn dynamic_metadata(&self) -> &DynamicMetadata {
        &self.dynamic_metadata
    }

    pub fn update_caption(
        &mut self,
        new_caption: Option<Caption>,
        new_mentions: Mentions,
    ) -> Result<bool> {
        if self.caption == new_caption {
            return Ok(false);
        }

        let new_tags = match &new_caption {
            Some(cap) => {
                Hashtags::try_from(cap.extract_hashtags().into_iter().collect::<Vec<String>>())?
            }
            None => Hashtags::empty(),
        };

        self.track_change(
            |s| {
                s.caption = new_caption;
                s.hashtags = new_tags;
                s.mentions = new_mentions;
                s.is_edited = true;
                s.updated_at = Some(Utc::now());
                Ok(true)
            },
            |s| {
                Box::new(PostEvent::PostCaptionUpdated {
                    post_id: s.post_id,
                    author_id: s.author_id,
                    occurred_at: s.updated_at.unwrap_or_else(Utc::now),
                })
            },
        )
    }

    pub fn toggle_comments(&mut self, allowed: bool) -> Result<bool> {
        if self.allowed_comment_hands == allowed {
            return Ok(false);
        }

        self.track_change(
            |s| {
                s.allowed_comment_hands = allowed;
                Ok(true)
            },
            |s| {
                Box::new(PostEvent::PostCommentsToggled {
                    post_id: s.post_id,
                    author_id: s.author_id,
                    allowed_comment_hands: allowed,
                    occurred_at: Utc::now(),
                })
            },
        )
    }

    pub fn change_visibility(&mut self, new_level: VisibilityLevel) -> Result<bool> {
        if self.visibility_level == new_level {
            return Ok(false);
        }

        self.track_change(
            |s| {
                s.visibility_level = new_level;
                Ok(true)
            },
            |s| {
                Box::new(PostEvent::PostVisibilityChanged {
                    post_id: s.post_id,
                    author_id: s.author_id,
                    new_visibility: new_level,
                    occurred_at: Utc::now(),
                })
            },
        )
    }

    pub fn validate_invariants(
        post_type: PostType,
        media: &[MediaAsset],
        caption: &Option<Caption>,
    ) -> Result<()> {
        Self::validate_media_invariants(post_type, media)?;

        if post_type == PostType::Text && caption.is_none() {
            return Err(Error::validation(
                "caption",
                "Text posts must have a caption".to_string(),
            ));
        }

        Ok(())
    }

    fn validate_media_invariants(post_type: PostType, media: &[MediaAsset]) -> Result<()> {
        match post_type {
            PostType::Text => {
                if !media.is_empty() {
                    return Err(Error::validation(
                        "post",
                        "Text posts cannot contain any media assets",
                    ));
                }
            }
            PostType::Image => {
                if media.len() != 1 || media[0].media_type().is_video() {
                    return Err(Error::validation(
                        "post",
                        "Image posts must contain exactly one image asset",
                    ));
                }
            }
            PostType::Video => {
                if media.len() != 1 || !media[0].media_type().is_video() {
                    return Err(Error::validation(
                        "post",
                        "Video posts must contain exactly one video asset",
                    ));
                }
            }
            PostType::Carousel => {
                if media.len() < 2 {
                    return Err(Error::validation(
                        "post",
                        "Carousel posts must contain at least two media assets",
                    ));
                }
            }
        }
        Ok(())
    }
}
