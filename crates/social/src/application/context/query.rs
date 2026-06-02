// crates/social/src/application/context/query.rs

use crate::application::context::SocialAppContext;
use crate::domain::entities::ProfileCounters;
use shared_kernel::core::ErrorCode;
use shared_kernel::{
    core::{Error, Result},
    types::{ProfileId, Region},
};

#[derive(Clone)]
pub struct SocialQueryContext {
    app: SocialAppContext,
    region: Region,
}

impl SocialQueryContext {
    pub(crate) fn new(app: SocialAppContext, region: Region) -> Self {
        Self { app, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn is_already_following(
        &self,
        follower_id: ProfileId,
        following_id: ProfileId,
    ) -> Result<bool> {
        self.app
            .relation_repo()
            .is_following(follower_id, following_id)
            .await
    }

    pub async fn get_following_list(
        &self,
        follower_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.app
            .relation_repo()
            .get_following_ids(follower_id, limit, offset)
            .await
    }

    pub async fn get_followers_list(
        &self,
        following_id: ProfileId,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProfileId>> {
        self.app
            .relation_repo()
            .get_followers_ids(following_id, limit, offset)
            .await
    }

    pub async fn get_profile_counters(&self, profile_id: ProfileId) -> Result<ProfileCounters> {
        match self.app.cache_counter_repo().get_counters(profile_id).await {
            Ok(counters) => Ok(counters),

            Err(Error {
                code: ErrorCode::NotFound,
                ..
            }) => {
                let db_counters = self.app.counter_repo().get_counters(profile_id).await?;
                if let Err(e) = self.app.cache_counter_repo().save(&db_counters).await {
                    tracing::warn!(
                        "Failed to warm up Redis counter cache for {}: {:?}",
                        profile_id,
                        e
                    );
                }

                Ok(db_counters)
            }

            Err(other_error) => Err(other_error),
        }
    }
}