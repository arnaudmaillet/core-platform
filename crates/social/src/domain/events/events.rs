use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_kernel::{messaging::Event, types::ProfileId};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SocialEvent {
    ProfileFollowed {
        id: Uuid,
        follower_id: ProfileId,
        following_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },

    ProfileUnfollowed {
        id: Uuid,
        follower_id: ProfileId,
        following_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },
}

impl SocialEvent {
    pub const USER_FOLLOWED: &'static str = "social.profile.followed";
    pub const USER_UNFOLLOWED: &'static str = "social.profile.unfollowed";
}

impl Event for SocialEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::ProfileFollowed { id, .. } | Self::ProfileUnfollowed { id, .. } => *id,
        }
    }

    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::ProfileFollowed { .. } => Self::USER_FOLLOWED,
            Self::ProfileUnfollowed { .. } => Self::USER_UNFOLLOWED,
        };
        Cow::Borrowed(s)
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("social_relation")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::ProfileFollowed {
                follower_id,
                following_id,
                ..
            }
            | Self::ProfileUnfollowed {
                follower_id,
                following_id,
                ..
            } => {
                format!("{}:{}", follower_id, following_id)
            }
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::ProfileFollowed { occurred_at, .. }
            | Self::ProfileUnfollowed { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
