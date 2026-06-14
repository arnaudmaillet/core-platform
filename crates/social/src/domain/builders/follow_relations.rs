// crates/social/src/domain/builders/follow_relation_builder.rs

use chrono::{DateTime, Utc};
use shared_kernel::{core::Result, types::ProfileId};

use crate::entities::FollowRelation;

pub struct FollowRelationBuilder {
    follower_id: ProfileId,
    following_id: ProfileId,
    created_at: Option<DateTime<Utc>>,
}

impl FollowRelationBuilder {
    pub fn new(follower_id: ProfileId, following_id: ProfileId) -> Self {
        Self {
            follower_id,
            following_id,
            created_at: None,
        }
    }

    pub fn with_created_at(mut self, date: DateTime<Utc>) -> Self {
        self.created_at = Some(date);
        self
    }

    pub fn build(self) -> Result<FollowRelation> {
        let now = Utc::now();
        let created_at = self.created_at.unwrap_or(now);

        Ok(FollowRelation::restore(
            self.follower_id,
            self.following_id,
            created_at,
            now,
        ))
    }
}
