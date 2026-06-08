// crates/content_comments/src/domain/builders/comment.rs

use crate::entities::Comment;
use crate::types::{CommentContent, CommentId};
use chrono::{DateTime, Utc};
use shared_kernel::core::{LifecycleTracker, Result};
use shared_kernel::types::{PostId, ProfileId};

pub struct CommentBuilder {
    comment_id: CommentId,
    post_id: PostId,
    profile_id: ProfileId,
    parent_comment_id: Option<CommentId>,
    content: CommentContent,
    created_at: Option<DateTime<Utc>>,
    edited_at: Option<DateTime<Utc>>,
}

impl CommentBuilder {
    pub fn new(post_id: PostId, profile_id: ProfileId, content: CommentContent) -> Result<Self> {
        Ok(Self {
            comment_id: CommentId::generate(),
            post_id,
            profile_id,
            parent_comment_id: None,
            content,
            created_at: None,
            edited_at: None,
        })
    }

    pub fn with_parent_comment_id(mut self, parent_id: Option<CommentId>) -> Self {
        self.parent_comment_id = parent_id;
        self
    }

    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = Some(created_at);
        self
    }

    pub fn with_edited_at(mut self, edited_at: Option<DateTime<Utc>>) -> Self {
        self.edited_at = edited_at;
        self
    }

    pub fn build(self) -> Result<Comment> {
        let now = Utc::now();
        let created_at = self.created_at.unwrap_or(now);

        Ok(Comment::restore(
            self.comment_id,
            self.post_id,
            self.profile_id,
            self.parent_comment_id,
            self.content,
            created_at,
            self.edited_at,
            LifecycleTracker::default(),
        ))
    }
}
