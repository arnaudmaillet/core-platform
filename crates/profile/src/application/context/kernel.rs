// crates/profile/src/context/app_context.rs

use crate::context::ProfileCommandCtx;
use crate::repositories::ProfileRepository;
use crate::repositories::ProfileRoutingRepository;
use shared_kernel::environment::ClusterContext;
use shared_kernel::idempotency::IdempotencyRepository;
use shared_kernel::types::Region;
use std::sync::Arc;

pub struct ProfileKernelCtx {
    profile_repo: Arc<dyn ProfileRepository>,
    routing_repo: Arc<dyn ProfileRoutingRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    cluster_ctx: ClusterContext,
}

impl ProfileKernelCtx {
    pub fn new(
        profile_repo: Arc<dyn ProfileRepository>,
        routing_repo: Arc<dyn ProfileRoutingRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            profile_repo,
            routing_repo,
            idempotency_repo,
            cluster_ctx,
        }
    }

    pub(crate) fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.profile_repo.clone()
    }

    pub(crate) fn routing_repo(&self) -> Arc<dyn ProfileRoutingRepository> {
        self.routing_repo.clone()
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }

    pub fn creation_command(&self, region: Region) -> ProfileCommandCtx {
        ProfileCommandCtx::new(self.clone(), None, region)
    }
}

impl Clone for ProfileKernelCtx {
    fn clone(&self) -> Self {
        Self {
            profile_repo: self.profile_repo.clone(),
            routing_repo: self.routing_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
            cluster_ctx: self.cluster_ctx,
        }
    }
}
