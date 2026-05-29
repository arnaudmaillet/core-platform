// crates/account/src/application/context.rs (ou son chemin équivalent)

use crate::application::context::{AccountCommandContext, AccountQueryContext};
use crate::domain::repositories::AccountRepository;
use shared_kernel::core::TransactionManager;
use shared_kernel::{
    idempotency::IdempotencyRepository,
    messaging::OutboxRepository,
    types::{AccountId, Region},
};
use std::sync::Arc;

pub struct AccountAppContext<TM> {
    transaction_manager: Arc<TM>,
    account_repo: Arc<dyn AccountRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl<TM> Clone for AccountAppContext<TM> {
    fn clone(&self) -> Self {
        Self {
            transaction_manager: self.transaction_manager.clone(),
            account_repo: self.account_repo.clone(),
            outbox_repo: self.outbox_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
        }
    }
}

impl<TM> AccountAppContext<TM> {
    pub fn new(
        transaction_manager: Arc<TM>,
        account_repo: Arc<dyn AccountRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            transaction_manager,
            account_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn transaction_manager(&self) -> Arc<TM> {
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
}

impl<TM: TransactionManager> AccountAppContext<TM> {
    pub fn query(&self, region: Region) -> AccountQueryContext<TM> {
        AccountQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, account_id: AccountId, region: Region) -> AccountCommandContext<TM> {
        AccountCommandContext::new(self.clone(), Some(account_id), region)
    }

    pub fn creation_command(&self, region: Region) -> AccountCommandContext<TM> {
        AccountCommandContext::new(self.clone(), None, region)
    }
}
