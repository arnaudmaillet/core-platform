// crates/social/src/application/context/app.rs

use crate::{
    context::{SocialCommandContext, SocialQueryContext},
    domain::repositories::{CounterRepository, RelationRepository},
};
use shared_kernel::{
    types::{ProfileId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct SocialAppContext {
    relation_repo: Arc<dyn RelationRepository>,
    cache_counter_repo: Arc<dyn CounterRepository>,
    counter_repo: Arc<dyn CounterRepository>,
}

impl SocialAppContext {
    pub fn new(
        relation_repo: Arc<dyn RelationRepository>,
        cache_counter_repo: Arc<dyn CounterRepository>,
        counter_repo: Arc<dyn CounterRepository>,
    ) -> Self {
        Self {
            relation_repo,
            cache_counter_repo,
            counter_repo,
        }
    }

    pub fn query(&self, region: Region) -> SocialQueryContext {
        SocialQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, target_profile_id: ProfileId, region: Region) -> SocialCommandContext {
        SocialCommandContext::new(self.clone(), target_profile_id, region)
    }

    pub(crate) fn relation_repo(&self) -> Arc<dyn RelationRepository> {
        self.relation_repo.clone()
    }

    pub(crate) fn cache_counter_repo(&self) -> Arc<dyn CounterRepository> {
        self.cache_counter_repo.clone()
    }

    pub(crate) fn counter_repo(&self) -> Arc<dyn CounterRepository> {
        self.counter_repo.clone()
    }
}
