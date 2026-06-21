// crates/account/src/application/context.rs

use crate::application::context::{AccountCommandCtx, AccountQueryCtx};
use crate::repositories::{AccountRepository, GlobalIdentityRegistry};
use shared_kernel::application::environment::ClusterContext;
use shared_kernel::core::TransactionManager;
use shared_kernel::{
    idempotency::IdempotencyRepository,
    messaging::OutboxRepository,
    types::{AccountId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AccountKernelCtx {
    transaction_manager: Arc<dyn TransactionManager>,
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
    global_registry: Arc<dyn GlobalIdentityRegistry>,
    cluster_ctx: ClusterContext,
}

impl AccountKernelCtx {
    pub fn new(
        transaction_manager: Arc<dyn TransactionManager>,
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
        global_registry: Arc<dyn GlobalIdentityRegistry>,
        cluster_ctx: ClusterContext,
    ) -> Self {
        Self {
            transaction_manager,
            account_repo,
            outbox_repo,
            idempotency_repo,
            global_registry,
            cluster_ctx,
        }
    }

    pub fn transaction_manager(&self) -> Arc<dyn TransactionManager> {
        self.transaction_manager.clone()
    }

    pub(crate) fn account_repo(&self) -> Arc<dyn AccountRepository> {
        self.account_repo.clone()
    }

    pub(crate) fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.outbox_repo.clone()
    }

    pub(crate) fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }

    pub(crate) fn global_registry(&self) -> Arc<dyn GlobalIdentityRegistry> {
        self.global_registry.clone()
    }

    pub fn cluster_region(&self) -> Region {
        self.cluster_ctx.region()
    }

    pub fn build_query_ctx(&self, region_query: Region) -> AccountQueryCtx {
        AccountQueryCtx::new(self.clone(), region_query)
    }

    pub fn build_command_ctx(
        &self,
        account_id: AccountId,
        region_cmd: Region,
    ) -> AccountCommandCtx {
        AccountCommandCtx::new(self.clone(), Some(account_id), region_cmd)
    }

    pub fn build_creation_command_ctx(&self, region_cmd: Region) -> AccountCommandCtx {
        AccountCommandCtx::new(self.clone(), None, region_cmd)
    }
}
