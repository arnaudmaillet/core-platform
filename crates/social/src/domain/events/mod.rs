mod events;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shared_kernel::{
    messaging::Event,
    types::ProfileId,
};
use std::borrow::Cow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SocialDomainEvent {
    /// Un utilisateur commence à en suivre un autre
    UserFollowed {
        id: Uuid,
        follower_id: ProfileId,
        following_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },

    /// Un utilisateur arrête de suivre un autre utilisateur
    UserUnfollowed {
        id: Uuid,
        follower_id: ProfileId,
        following_id: ProfileId,
        occurred_at: DateTime<Utc>,
    },
}

impl SocialDomainEvent {
    pub const USER_FOLLOWED: &'static str = "social.user.followed";
    pub const USER_UNFOLLOWED: &'static str = "social.user.unfollowed";
}

impl Event for SocialDomainEvent {
    fn event_id(&self) -> Uuid {
        match self {
            Self::UserFollowed { id, .. } | Self::UserUnfollowed { id, .. } => *id,
        }
    }

    fn event_name(&self) -> Cow<'_, str> {
        let s = match self {
            Self::UserFollowed { .. } => Self::USER_FOLLOWED,
            Self::UserUnfollowed { .. } => Self::USER_UNFOLLOWED,
        };
        Cow::Borrowed(s)
    }

    fn region_code(&self) -> &str {
        "EU" 
    }

    fn aggregate_type(&self) -> Cow<'_, str> {
        Cow::Borrowed("social_relation")
    }

    fn aggregate_id(&self) -> String {
        match self {
            Self::UserFollowed { follower_id, following_id, .. }
            | Self::UserUnfollowed { follower_id, following_id, .. } => {
                format!("{}:{}", follower_id, following_id)
            }
        }
    }

    fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::UserFollowed { occurred_at, .. } | Self::UserUnfollowed { occurred_at, .. } => *occurred_at,
        }
    }

    fn payload(&self) -> Value {
        json!(self)
    }
}