// crates/social/src/application/builder.rs

use shared_kernel::command::CommandBus;

use crate::{
    context::{SocialCommandCtx, SocialKernelCtx},
    domain::repositories::{FollowRelationRepository, ProfileCountersIndexRepository},
    use_cases::{FollowCommand, FollowHandler, UnfollowCommand, UnfollowHandler},
};
use std::sync::Arc;

pub struct SocialServiceBuilder {
    follow_relation_repo: Arc<dyn FollowRelationRepository>,
    profile_counters_index: Arc<dyn ProfileCountersIndexRepository>,
}

impl SocialServiceBuilder {
    pub fn new(
        follow_relation_repo: Arc<dyn FollowRelationRepository>,
        profile_counters_index: Arc<dyn ProfileCountersIndexRepository>,
    ) -> Self {
        Self {
            follow_relation_repo,
            profile_counters_index,
        }
    }

    pub async fn build_context(&self) -> SocialKernelCtx {
        SocialKernelCtx::new(
            self.follow_relation_repo.clone(),
            self.profile_counters_index.clone(),
        )
    }

    pub fn register_handlers(&self, bus: &mut CommandBus) {
        bus.register::<SocialCommandCtx, FollowCommand, FollowHandler>(FollowHandler);
        bus.register::<SocialCommandCtx, UnfollowCommand, UnfollowHandler>(UnfollowHandler);
    }
}
