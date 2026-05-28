use crate::application::context::{AccountCommandContext, AccountQueryContext};
use crate::domain::repositories::AccountRepository;
use infra_sqlx::sqlx::PgPool;
use shared_kernel::{
    idempotency::IdempotencyRepository,
    messaging::OutboxRepository,
    types::{AccountId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AccountAppContext {
    pool: Option<PgPool>,
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl AccountAppContext {
    pub fn new(
        pool: PgPool,
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: Some(pool),
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn new_stubbed(
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: None,
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn query(&self, region: Region) -> AccountQueryContext {
        AccountQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, account_id: AccountId, region: Region) -> AccountCommandContext {
        AccountCommandContext::new(self.clone(), Some(account_id), region)
    }

    pub fn creation_command(&self, region: Region) -> AccountCommandContext {
        AccountCommandContext::new(self.clone(), None, region)
    }

    pub(crate) fn pg_pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
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
}
