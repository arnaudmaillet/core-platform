// crates/profile/src/context/app_context.rs

use crate::context::ProfileCommandContext;
use crate::context::ProfileQueryContext;
use crate::repositories::ProfileRepository;
use crate::repositories::ProfileRoutingRepository;
use shared_kernel::types::{ProfileId, Region};
use std::sync::Arc;

pub struct ProfileAppContext {
    profile_repo: Arc<dyn ProfileRepository>,
    routing_repo: Arc<dyn ProfileRoutingRepository>,
    local_region: Region,
}

impl ProfileAppContext {
    pub fn new(
        profile_repo: Arc<dyn ProfileRepository>,
        routing_repo: Arc<dyn ProfileRoutingRepository>,
        local_region: Region,
    ) -> Self {
        Self {
            profile_repo,
            routing_repo,
            local_region,
        }
    }

    pub(crate) fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.profile_repo.clone()
    }

    pub(crate) fn routing_repo(&self) -> Arc<dyn ProfileRoutingRepository> {
        self.routing_repo.clone()
    }

    pub fn local_region(&self) -> Region {
        self.local_region
    }

    pub fn query(&self) -> ProfileQueryContext {
        ProfileQueryContext::new(self.clone())
    }

    pub fn command(&self, profile_id: ProfileId) -> ProfileCommandContext {
        ProfileCommandContext::new(self.clone(), Some(profile_id))
    }

    pub fn creation_command(&self) -> ProfileCommandContext {
        ProfileCommandContext::new(self.clone(), None)
    }
}

impl Clone for ProfileAppContext {
    fn clone(&self) -> Self {
        Self {
            profile_repo: self.profile_repo.clone(),
            routing_repo: self.routing_repo.clone(),
            local_region: self.local_region,
        }
    }
}
