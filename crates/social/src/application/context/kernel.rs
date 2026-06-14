// crates/social/src/application/context/app.rs

// crates/social/src/application/context/app.rs

use crate::domain::repositories::{FollowRelationRepository, ProfileCountersIndexRepository};
use std::sync::Arc;

#[derive(Clone)]
pub struct SocialKernelCtx {
    follow_relation_repo: Arc<dyn FollowRelationRepository>,
    profile_counters_index: Arc<dyn ProfileCountersIndexRepository>,
}

impl SocialKernelCtx {
    pub fn new(
        follow_relation_repo: Arc<dyn FollowRelationRepository>,
        profile_counters_index: Arc<dyn ProfileCountersIndexRepository>,
    ) -> Self {
        Self {
            follow_relation_repo,
            profile_counters_index,
        }
    }

    pub(crate) fn follow_relation_repo(&self) -> Arc<dyn FollowRelationRepository> {
        self.follow_relation_repo.clone()
    }

    pub(crate) fn profile_counters_index(&self) -> Arc<dyn ProfileCountersIndexRepository> {
        self.profile_counters_index.clone()
    }
}
