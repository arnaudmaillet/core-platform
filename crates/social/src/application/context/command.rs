// crates/social/src/application/context/command.rs

use crate::application::context::SocialAppContext;
use crate::entities::FollowRelation;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};

#[derive(Clone)]
pub struct SocialCommandContext {
    app: SocialAppContext,
    target_profile_id: ProfileId,
    region: Region,
}

impl SocialCommandContext {
    pub(crate) fn new(app: SocialAppContext, target_profile_id: ProfileId, region: Region) -> Self {
        Self {
            app,
            target_profile_id,
            region,
        }
    }

    pub fn app(&self) -> &SocialAppContext {
        &self.app
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn target_profile_id(&self) -> ProfileId {
        self.target_profile_id
    }

    pub fn verify_actors(&self, follower_id: ProfileId, target_id: ProfileId) -> Result<()> {
        if target_id != self.target_profile_id {
            return Err(Error::validation(
                "target",
                "Context/Target mismatch violation",
            ));
        }
        Ok(())
    }

    pub async fn save_relation(&self, relation: &mut FollowRelation) -> Result<()> {
        if relation.following_id() != self.target_profile_id {
            return Err(Error::validation(
                "following_id",
                "Identity mismatch violation",
            ));
        }

        self.app.relation_repo().save(relation).await?;
        self.app
            .cache_counter_repo()
            .increment_counters(relation.follower_id(), relation.following_id())
            .await?;
        Ok(())
    }

    pub async fn delete_relation(&self, relation: &mut FollowRelation) -> Result<()> {
        self.app.relation_repo().delete(relation).await?;
        self.app
            .cache_counter_repo()
            .decrement_counters(relation.follower_id(), relation.following_id())
            .await?;
        Ok(())
    }
}
