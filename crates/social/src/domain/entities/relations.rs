// crates/social/src/domain/aggregates/follow_relation.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{Entity, LifecycleTracker, ManagedEntity, Result};
use shared_kernel::messaging::{Event, EventEmitter, OperationTracker};
use shared_kernel::types::ProfileId;
use uuid::Uuid;

use crate::domain::events::SocialEvent;
use crate::domain::types::FollowRelationId;
use crate::entities::FollowRelationBuilder;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FollowRelation {
    id: FollowRelationId,
    created_at: DateTime<Utc>,
    metadata: LifecycleTracker,
}

impl EventEmitter for FollowRelation {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl ManagedEntity for FollowRelation {
    fn lifecycle(&self) -> &LifecycleTracker {
        &self.metadata
    }
    fn lifecycle_mut(&mut self) -> &mut LifecycleTracker {
        &mut self.metadata
    }
}

impl Entity for FollowRelation {
    type Id = FollowRelationId;

    fn entity_name() -> &'static str {
        "FollowRelation"
    }

    fn map_constraint_to_field(_constraint: &str) -> &'static str {
        "follower_id_following_id"
    }

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
}

impl FollowRelation {
    pub fn builder(follower_id: ProfileId, following_id: ProfileId) -> FollowRelationBuilder {
        FollowRelationBuilder::new(follower_id, following_id)
    }

    pub fn restore(
        follower_id: ProfileId,
        following_id: ProfileId,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        FollowRelation {
            id: FollowRelationId::new(follower_id, following_id), // Construit à la restauration
            created_at,
            metadata: LifecycleTracker::restore(updated_at),
        }
    }

    pub fn follower_id(&self) -> ProfileId {
        self.id.follower_id()
    }
    pub fn following_id(&self) -> ProfileId {
        self.id.following_id()
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn execute_follow(&mut self) -> Result<bool> {
        OperationTracker::track_change(
            self,
            |_s| Ok(true),
            |s| {
                Box::new(SocialEvent::ProfileFollowed {
                    id: Uuid::now_v7(),
                    follower_id: s.follower_id(),
                    following_id: s.following_id(),
                    occurred_at: s.metadata.updated_at(),
                })
            },
        )
    }

    pub fn execute_unfollow(&mut self) -> Result<bool> {
        OperationTracker::track_change(
            self,
            |_s| Ok(true),
            |s| {
                Box::new(SocialEvent::ProfileUnfollowed {
                    id: Uuid::now_v7(),
                    follower_id: s.follower_id(),
                    following_id: s.following_id(),
                    occurred_at: s.metadata.updated_at(),
                })
            },
        )
    }
}
