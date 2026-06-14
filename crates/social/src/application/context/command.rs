// crates/social/src/application/context/command.rs

use crate::application::context::SocialKernelCtx;
use crate::domain::entities::FollowRelation;
use shared_kernel::core::{Error, Result};
use shared_kernel::types::{ProfileId, Region};

#[derive(Clone)]
pub struct SocialCommandCtx {
    kernel: SocialKernelCtx,
    target_profile_id: ProfileId,
    region: Region,
}

impl SocialCommandCtx {
    pub fn new(kernel: SocialKernelCtx, target_profile_id: ProfileId, region: Region) -> Self {
        Self {
            kernel,
            target_profile_id,
            region,
        }
    }

    pub fn kernel(&self) -> &SocialKernelCtx {
        &self.kernel
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub fn target_profile_id(&self) -> ProfileId {
        self.target_profile_id
    }

    pub fn verify_actors(&self, _follower_id: ProfileId, target_id: ProfileId) -> Result<()> {
        if target_id != self.target_profile_id {
            return Err(Error::validation(
                "target",
                "Context/Target mismatch violation",
            ));
        }
        Ok(())
    }

    pub async fn is_already_following(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<bool> {
        self.kernel
            .follow_relation_repo()
            .is_following(follower_id, following_id)
            .await
    }

    pub async fn save_relation(&self, relation: &mut FollowRelation) -> Result<()> {
        if relation.following_id() != self.target_profile_id {
            return Err(Error::validation(
                "following_id",
                "Identity mismatch violation",
            ));
        }

        self.kernel.follow_relation_repo().save(relation).await?;

        self.kernel
            .profile_counters_index()
            .increment(relation.follower_id(), relation.following_id())
            .await?;

        Ok(())
    }

    pub async fn delete_relation(&self, relation: &mut FollowRelation) -> Result<()> {
        self.kernel.follow_relation_repo().delete(relation).await?;

        self.kernel
            .profile_counters_index()
            .decrement(relation.follower_id(), relation.following_id())
            .await?;

        Ok(())
    }
}
