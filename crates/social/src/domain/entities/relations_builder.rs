use chrono::{DateTime, Utc};
use shared_kernel::{core::Result, types::ProfileId};

use crate::entities::FollowRelation;

pub struct FollowRelationBuilder {
    follower_id: ProfileId,
    following_id: ProfileId,
    created_at: Option<DateTime<Utc>>,
    version: u64,
}

impl FollowRelationBuilder {
    pub(crate) fn new(follower_id: ProfileId, following_id: ProfileId) -> Self {
        Self {
            follower_id,
            following_id,
            created_at: None,
            version: 1,
        }
    }

    pub fn with_created_at(mut self, date: DateTime<Utc>) -> Self {
        self.created_at = Some(date);
        self
    }

    pub fn with_version(mut self, version: u64) -> Self {
        self.version = version;
        self
    }

    /// Construit l'agrégat final
    pub fn build(self) -> Result<FollowRelation> {
        let now = Utc::now();
        let created_at = self.created_at.unwrap_or(now);

        Ok(FollowRelation::restore(
            self.follower_id,
            self.following_id,
            self.version,
            created_at,
            now,
        ))
    }
}
