// crates/content_comments/src/domain/aggregates/comment.rs

use crate::entities::CommentBuilder;
use crate::events::CommentEvent;
use crate::types::{CommentContent, CommentId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Entity, Error, LifecycleTracker, ManagedEntity, Result};
use shared_kernel::messaging::{Event, EventEmitter, OperationTracker};
use shared_kernel::types::{PostId, ProfileId};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Comment {
    comment_id: CommentId,
    post_id: PostId,
    profile_id: ProfileId,
    parent_comment_id: Option<CommentId>,
    content: CommentContent,
    created_at: DateTime<Utc>,
    edited_at: Option<DateTime<Utc>>,
    lifecycle: LifecycleTracker,
}

impl EventEmitter for Comment {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.lifecycle.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.lifecycle.pull_events()
    }
}

impl Entity for Comment {
    type Id = CommentId;

    fn entity_name() -> &'static str {
        "Comment"
    }

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "comment_id"
    }

    fn id(&self) -> &Self::Id {
        self.comment_id_as_ref()
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.lifecycle.updated_at()
    }
}

impl ManagedEntity for Comment {
    fn lifecycle(&self) -> &LifecycleTracker {
        &self.lifecycle
    }
    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker {
        &mut self.lifecycle
    }
}

impl Comment {
    pub fn builder(
        post_id: PostId,
        profile_id: ProfileId,
        content: CommentContent,
    ) -> Result<CommentBuilder> {
        CommentBuilder::new(post_id, profile_id, content)
    }

    pub fn restore(
        comment_id: CommentId,
        post_id: PostId,
        profile_id: ProfileId,
        parent_comment_id: Option<CommentId>,
        content: CommentContent,
        created_at: DateTime<Utc>,
        edited_at: Option<DateTime<Utc>>,
        lifecycle: LifecycleTracker,
    ) -> Self {
        Self {
            comment_id,
            post_id,
            profile_id,
            parent_comment_id,
            content,
            created_at,
            edited_at,
            lifecycle,
        }
    }

    pub(crate) fn comment_id_as_ref(&self) -> &CommentId {
        &self.comment_id
    }

    pub fn comment_id(&self) -> CommentId {
        self.comment_id
    }

    pub fn post_id(&self) -> PostId {
        self.post_id
    }

    pub fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    pub fn parent_comment_id(&self) -> Option<CommentId> {
        self.parent_comment_id
    }

    pub fn content(&self) -> &CommentContent {
        &self.content
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn edited_at(&self) -> Option<DateTime<Utc>> {
        self.edited_at
    }

    pub fn publish_comment(&mut self) -> Result<bool> {
        OperationTracker::track_change(
            self,
            |_s| Ok(true),
            |s| {
                let event_id = Uuid::now_v7();
                if let Some(parent_id) = s.parent_comment_id() {
                    Box::new(CommentEvent::ReplyCreated {
                        id: event_id,
                        comment_id: s.comment_id(),
                        parent_comment_id: parent_id,
                        post_id: s.post_id(),
                        profile_id: s.profile_id(),
                        content: s.content.clone(),
                        occurred_at: s.lifecycle.updated_at(),
                    })
                } else {
                    Box::new(CommentEvent::CommentCreated {
                        id: event_id,
                        comment_id: s.comment_id(),
                        post_id: s.post_id(),
                        profile_id: s.profile_id(),
                        content: s.content.clone(),
                        occurred_at: s.lifecycle.updated_at(),
                    })
                }
            },
        )
    }

    pub fn edit_content(
        &mut self,
        editor_id: ProfileId,
        new_content: CommentContent,
    ) -> Result<bool> {
        if self.profile_id != editor_id {
            return Err(Error::forbidden(&format!(
                "Le profil {} n'a pas les droits pour éditer le commentaire de {}",
                editor_id, self.profile_id
            )));
        }

        if self.content == new_content {
            return Ok(false);
        }

        OperationTracker::track_change(
            self,
            |s| {
                s.content = new_content;
                s.edited_at = Some(Utc::now());
                Ok(true)
            },
            |s| {
                Box::new(CommentEvent::CommentEdited {
                    id: Uuid::now_v7(),
                    comment_id: s.comment_id(),
                    content: s.content.clone(),
                    occurred_at: s.edited_at().unwrap(),
                })
            },
        )
    }

    pub fn delete_content(&mut self, editor_id: ProfileId) -> Result<bool> {
        if self.profile_id != editor_id {
            return Err(Error::forbidden(&format!(
                "Le profil {} n'a pas les droits pour supprimer le commentaire de {}",
                editor_id, self.profile_id
            )));
        }

        let placeholder = CommentContent::try_new("[Commentaire supprimé]")?;
        if self.content == placeholder {
            return Ok(false);
        }

        OperationTracker::track_change(
            self,
            |s| {
                s.content = placeholder;
                Ok(true)
            },
            |s| {
                Box::new(CommentEvent::CommentDeleted {
                    id: Uuid::now_v7(),
                    comment_id: s.comment_id(),
                    post_id: s.post_id(),
                    profile_id: s.profile_id(),
                    occurred_at: s.lifecycle.updated_at(),
                })
            },
        )
    }
}
