use crate::{
    context::{ProfileCommandContext, ProfileQueryContext},
    repositories::ProfileRepository,
};
use shared_kernel::{
    core::TransactionManager,
    idempotency::IdempotencyRepository,
    messaging::OutboxRepository,
    types::{ProfileId, Region},
};
use std::sync::Arc;

pub struct ProfileAppContext<TM> {
    transaction_manager: Arc<TM>,
    profile_repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl<TM> ProfileAppContext<TM> {
    pub fn new(
        transaction_manager: Arc<TM>,
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            transaction_manager,
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn transaction_manager(&self) -> Arc<TM> {
        self.transaction_manager.clone()
    }
    pub(crate) fn profile_repo(&self) -> Arc<dyn ProfileRepository> {
        self.profile_repo.clone()
    }
    pub(crate) fn outbox_repo(&self) -> Arc<dyn OutboxRepository> {
        self.outbox_repo.clone()
    }
    pub(crate) fn idempotency_repo(&self) -> Arc<dyn IdempotencyRepository> {
        self.idempotency_repo.clone()
    }
}

impl<TM: TransactionManager> ProfileAppContext<TM> {
    pub fn query(&self, region: Region) -> ProfileQueryContext<TM> {
        ProfileQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, profile_id: ProfileId, region: Region) -> ProfileCommandContext<TM> {
        ProfileCommandContext::new(self.clone(), Some(profile_id), region)
    }

    pub fn creation_command(&self, region: Region) -> ProfileCommandContext<TM> {
        ProfileCommandContext::new(self.clone(), None, region)
    }
}

impl<TM> Clone for ProfileAppContext<TM> {
    fn clone(&self) -> Self {
        Self {
            transaction_manager: self.transaction_manager.clone(),
            profile_repo: self.profile_repo.clone(),
            outbox_repo: self.outbox_repo.clone(),
            idempotency_repo: self.idempotency_repo.clone(),
        }
    }
}