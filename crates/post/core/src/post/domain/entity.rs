// crates/post/src/domain/aggregates/post.rs

use crate::{
    Media,
    post::domain::{
        PostBuilder, PostEvent,
        types::{Caption, DynamicMetadata, Hashtags, Mentions, VisibilityLevel},
    },
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::{
    core::{Entity, Error, LifecycleTracker, ManagedEntity, Result, Versioned},
    messaging::{Event, EventEmitter, OperationTracker},
    types::{MusicId, PostId, PostType, ProfileId},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Post {
    post_id: PostId,
    author_id: ProfileId,
    post_type: PostType,
    caption: Option<Caption>,
    media_list: Vec<Media>,
    total_duration_seconds: u32,
    allowed_comment_hands: bool,
    visibility_level: VisibilityLevel,
    music_id: Option<MusicId>,
    hashtags: Hashtags,
    mentions: Mentions,
    dynamic_metadata: DynamicMetadata,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    lifecycle: LifecycleTracker,
    version: u64,
}

impl Versioned for Post {
    fn version(&self) -> u64 {
        self.version
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
    fn record_change(&mut self) {
        self.version += 1;
        self.lifecycle.record_change();
    }
}

impl EventEmitter for Post {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.lifecycle.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.lifecycle.pull_events()
    }
}

impl ManagedEntity for Post {
    fn lifecycle(&self) -> &LifecycleTracker {
        &self.lifecycle
    }
    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker {
        &mut self.lifecycle
    }
}

impl Entity for Post {
    type Id = PostId;

    fn entity_name() -> &'static str {
        "Post"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "posts_pkey" => "post_id",
            _ => "internal_security",
        }
    }

    fn id(&self) -> &Self::Id {
        &self.post_id
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
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

    pub(crate) fn restore(
        post_id: PostId,
        author_id: ProfileId,
        post_type: PostType,
        caption: Option<Caption>,
        media_list: Vec<Media>,
        total_duration_seconds: u32,
        allowed_comment_hands: bool,
        visibility_level: VisibilityLevel,
        music_id: Option<MusicId>,
        hashtags: Hashtags,
        mentions: Mentions,
        dynamic_metadata: DynamicMetadata,
        created_at: DateTime<Utc>,
        edited_at: Option<DateTime<Utc>>,
        lifecycle: LifecycleTracker,
        version: u64,
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
            dynamic_metadata,
            created_at,
            edited_at,
            lifecycle,
            version,
        }
    }

    fn track_change<F, E>(&mut self, action: F, event_factory: E) -> Result<bool>
    where
        F: FnOnce(&mut Self) -> Result<bool>,
        E: FnOnce(&Self) -> Box<dyn Event>,
    {
        let changed = OperationTracker::track_change(self, action, event_factory)?;

        if changed {
            self.record_change();
        }

        Ok(changed)
    }

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
    pub fn media_list(&self) -> &[Media] {
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
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn edited_at(&self) -> Option<DateTime<Utc>> {
        self.edited_at
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
                s.edited_at = Some(Utc::now());
                Ok(true)
            },
            |s| {
                Box::new(PostEvent::PostCaptionUpdated {
                    post_id: s.post_id,
                    author_id: s.author_id,
                    occurred_at: s.lifecycle().updated_at(),
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
                    occurred_at: s.lifecycle().updated_at(),
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
                    occurred_at: s.lifecycle().updated_at(),
                })
            },
        )
    }

    pub fn validate_invariants(
        post_type: PostType,
        media: &[Media],
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

    fn validate_media_invariants(post_type: PostType, media: &[Media]) -> Result<()> {
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
