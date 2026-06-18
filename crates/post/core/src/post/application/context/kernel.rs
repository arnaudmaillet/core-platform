// crates/post/core/src/post/application/context/kernel.rs

use crate::post::domain::repositories::{PostReadRepository, PostWriteRepository};
use shared_kernel::{environment::ClusterContext, types::Region};
use std::sync::Arc;

#[derive(Clone)]
pub struct PostKernelCtx {
    read_repo: Arc<dyn PostReadRepository>,
    write_repo: Arc<dyn PostWriteRepository>,
    cluster_ctx: ClusterContext,
}

impl PostKernelCtx {
    pub fn new(
        read_repo: Arc<dyn PostReadRepository>,
        write_repo: Arc<dyn PostWriteRepository>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            read_repo,
            write_repo,
            cluster_ctx,
        }
    }

    pub fn server_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn read_repo(&self) -> Arc<dyn PostReadRepository> {
        self.read_repo.clone()
    }

    pub fn write_repo(&self) -> Arc<dyn PostWriteRepository> {
        self.write_repo.clone()
    }
}
