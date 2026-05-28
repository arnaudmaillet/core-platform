use crate::{
    context::{ProfileCommandContext, ProfileQueryContext},
    repositories::ProfileRepository,
};
use infra_sqlx::sqlx::PgPool;
use shared_kernel::{
    idempotency::IdempotencyRepository,
    messaging::OutboxRepository,
    types::{ProfileId, Region},
};
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileAppContext {
    pool: Option<PgPool>,
    profile_repo: Arc<dyn ProfileRepository>,
    outbox_repo: Arc<dyn OutboxRepository>,
    idempotency_repo: Arc<dyn IdempotencyRepository>,
}

impl ProfileAppContext {
    pub fn new(
        pool: PgPool,
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: Some(pool),
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    #[cfg(any(test, feature = "test-utils"))]
    pub fn new_stubbed(
        profile_repo: Arc<dyn ProfileRepository>,
        outbox_repo: Arc<dyn OutboxRepository>,
        idempotency_repo: Arc<dyn IdempotencyRepository>,
    ) -> Self {
        Self {
            pool: None,
            profile_repo,
            outbox_repo,
            idempotency_repo,
        }
    }

    pub fn query(&self, region: Region) -> ProfileQueryContext {
        ProfileQueryContext::new(self.clone(), region)
    }

    pub fn command(&self, profile_id: ProfileId, region: Region) -> ProfileCommandContext {
        ProfileCommandContext::new(self.clone(), Some(profile_id), region)
    }

    pub fn creation_command(&self, region: Region) -> ProfileCommandContext {
        ProfileCommandContext::new(self.clone(), None, region)
    }

    pub(crate) fn pg_pool(&self) -> Option<&PgPool> {
        self.pool.as_ref()
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
