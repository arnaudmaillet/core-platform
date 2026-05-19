use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shared_kernel::core::{AggregateMetadata, AggregateRoot, Entity, Result, Versioned};
use shared_kernel::messaging::{Event, EventEmitter, OperationTracker};
use shared_kernel::types::ProfileId;
use uuid::Uuid;

use crate::entities::FollowRelationBuilder;
use crate::events::SocialEvent;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FollowRelation {
    follower_id: ProfileId,
    following_id: ProfileId,
    created_at: DateTime<Utc>,
    metadata: AggregateMetadata,
}

impl Versioned for FollowRelation {
    fn version(&self) -> u64 {
        self.metadata.version()
    }
    fn updated_at(&self) -> DateTime<Utc> {
        self.metadata.updated_at()
    }
    fn record_change(&mut self) {
        self.metadata.record_change();
    }
}

impl EventEmitter for FollowRelation {
    fn push_event(&mut self, event: Box<dyn Event>) {
        self.metadata.push_event(event);
    }
    fn pull_events(&mut self) -> Vec<Box<dyn Event>> {
        self.metadata.pull_events()
    }
}

impl AggregateRoot for FollowRelation {
    fn id(&self) -> String {
        format!("{}:{}", self.follower_id, self.following_id)
    }
    fn metadata(&self) -> &AggregateMetadata {
        &self.metadata
    }
    fn metadata_mut(&mut self) -> &mut AggregateMetadata {
        &mut self.metadata
    }
}

impl Entity for FollowRelation {
    type Id = ProfileId;

    fn entity_name() -> &'static str {
        "FollowRelation"
    }

    fn map_constraint_to_field(constraint: &str) -> &'static str {
        match constraint {
            "social_relations_pkey" => "follower_id_following_id",
            _ => "internal_governance",
        }
    }

    fn id(&self) -> &Self::Id {
        &self.follower_id
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
        version: u64,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        FollowRelation {
            follower_id,
            following_id,
            created_at,
            metadata: AggregateMetadata::restore(version, updated_at),
        }
    }

    // --- GETTERS ---

    pub fn follower_id(&self) -> &ProfileId {
        &self.follower_id
    }
    pub fn following_id(&self) -> &ProfileId {
        &self.following_id
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        Versioned::updated_at(self)
    }

    // --- MUTATEURS MÉTIERS ---

    /// Exécute l'action de follow et enregistre l'événement de domaine
    pub fn execute_follow(&mut self) -> Result<bool> {
        self.track_change(
            |_s| Ok(true),
            |s| {
                Box::new(SocialEvent::ProfileFollowed {
                    id: Uuid::now_v7(),
                    follower_id: s.follower_id,
                    following_id: s.following_id,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }

    /// Exécute l'action d'unfollow et enregistre l'événement de domaine
    pub fn execute_unfollow(&mut self) -> Result<bool> {
        self.track_change(
            |_s| Ok(true),
            |s| {
                Box::new(SocialEvent::ProfileUnfollowed {
                    id: Uuid::now_v7(),
                    follower_id: s.follower_id,
                    following_id: s.following_id,
                    occurred_at: s.updated_at(),
                })
            },
        )
    }
}
