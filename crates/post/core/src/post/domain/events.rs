// crates/post/src/domain/events/post_event.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_kernel::messaging::Event;
use shared_kernel::types::{PostId, PostType, ProfileId, Region};
use std::borrow::Cow;
use uuid::Uuid;

use crate::post::domain::types::VisibilityLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum PostEvent {
    PostCreated {
        post_id: PostId,
        author_id: ProfileId,
        post_type: PostType,
        region: Region,
        occurred_at: DateTime<Utc>,
    },
    PostDeleted {
        post_id: PostId,
        author_id: ProfileId,
        region: Region,
        occurred_at: DateTime<Utc>,
    },

    PostCaptionUpdated {
        post_id: PostId,
        author_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },
    PostCommentsToggled {
        post_id: PostId,
        author_id: ProfileId,
        allowed_comment_hands: bool,
        occurred_at: DateTime<Utc>,
    },
    PostVisibilityChanged {
        post_id: PostId,
        author_id: ProfileId,
        new_visibility: VisibilityLevel,
        occurred_at: DateTime<Utc>,
    },
}

impl PostEvent {
    pub const CREATED: &'static str = "post.lifecycle.created";
    pub const DELETED: &'static str = "post.lifecycle.deleted";
    pub const CAPTION_EDITED: &'static str = "post.mutation.caption_edited";
    pub const COMMENTS_TOGGLED: &'static str = "post.mutation.comments_toggled";
    pub const VISIBILITY_CHANGED: &'static str = "post.mutation.visibility_changed";
}

impl Event for PostEvent {
    fn event_id(&self) -> Uuid {
        Uuid::now_v7()
    }

    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::PostCreated { .. } => Self::CREATED,
            Self::PostDeleted { .. } => Self::DELETED,
            Self::PostCaptionUpdated { .. } => Self::CAPTION_EDITED,
            Self::PostCommentsToggled { .. } => Self::COMMENTS_TOGGLED,
            Self::PostVisibilityChanged { .. } => Self::VISIBILITY_CHANGED,
        };
        Cow::Borrowed(s)
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("post")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::PostCreated { post_id, .. }
            | Self::PostDeleted { post_id, .. }
            | Self::PostCaptionUpdated { post_id, .. }
            | Self::PostCommentsToggled { post_id, .. }
            | Self::PostVisibilityChanged { post_id, .. } => post_id.to_string(),
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::PostCreated { occurred_at, .. }
            | Self::PostDeleted { occurred_at, .. }
            | Self::PostCaptionUpdated { occurred_at, .. }
            | Self::PostCommentsToggled { occurred_at, .. }
            | Self::PostVisibilityChanged { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
