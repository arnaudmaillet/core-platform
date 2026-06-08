// crates/content_comments/src/domain/events.rs

use crate::types::{CommentContent, CommentId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_kernel::{
    messaging::Event,
    types::{PostId, ProfileId},
};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CommentEvent {
    CommentCreated {
        id: Uuid,
        comment_id: CommentId,
        post_id: PostId,
        profile_id: ProfileId,
        content: CommentContent,
        occurred_at: DateTime<Utc>,
    },

    ReplyCreated {
        id: Uuid,
        comment_id: CommentId,
        parent_comment_id: CommentId,
        post_id: PostId,
        profile_id: ProfileId,
        content: CommentContent,
        occurred_at: DateTime<Utc>,
    },

    CommentEdited {
        id: Uuid,
        comment_id: CommentId,
        content: CommentContent,
        occurred_at: DateTime<Utc>,
    },

    CommentDeleted {
        id: Uuid,
        comment_id: CommentId,
        post_id: PostId,
        profile_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },
}

impl CommentEvent {
    pub const COMMENT_CREATED: &'static str = "comment.created";
    pub const REPLY_CREATED: &'static str = "comment.reply.created";
    pub const COMMENT_EDITED: &'static str = "comment.edited";
    pub const COMMENT_DELETED: &'static str = "comment.deleted";
}

impl Event for CommentEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::CommentCreated { id, .. }
            | Self::ReplyCreated { id, .. }
            | Self::CommentEdited { id, .. }
            | Self::CommentDeleted { id, .. } => *id,
        }
    }

    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::CommentCreated { .. } => Self::COMMENT_CREATED,
            Self::ReplyCreated { .. } => Self::REPLY_CREATED,
            Self::CommentEdited { .. } => Self::COMMENT_EDITED,
            Self::CommentDeleted { .. } => Self::COMMENT_DELETED,
        };
        Cow::Borrowed(s)
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("comment")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::CommentCreated { comment_id, .. }
            | Self::ReplyCreated { comment_id, .. }
            | Self::CommentEdited { comment_id, .. }
            | Self::CommentDeleted { comment_id, .. } => comment_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::CommentCreated { occurred_at, .. }
            | Self::ReplyCreated { occurred_at, .. }
            | Self::CommentEdited { occurred_at, .. }
            | Self::CommentDeleted { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }
}
