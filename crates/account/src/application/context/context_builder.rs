use std::sync::Arc;
use shared_kernel::domain::{repositories::OutboxRepository, value_objects::{AccountId, RegionCode}};
use crate::{application::context::AccountContext, domain::repositories::{AccountIdentityRepository, AccountMetadataRepository, AccountSettingsRepository}};




pub struct AccountContextBuilder {
    account_id: Option<AccountId>,
    region: Option<RegionCode>,
    identity_repo: Option<Arc<dyn AccountIdentityRepository>>,
    metadata_repo: Option<Arc<dyn AccountMetadataRepository>>,
    settings_repo: Option<Arc<dyn AccountSettingsRepository>>,
    outbox_repo: Option<Arc<dyn OutboxRepository>>,
    pool: Option<sqlx::PgPool>,
}

impl AccountContextBuilder {
    pub(crate) fn new() -> Self {
        Self {
            account_id: None,
            region: None,
            identity_repo: None,
            metadata_repo: None,
            settings_repo: None,
            outbox_repo: None,
            pool: None,
        }
    }

    pub fn has_identity_repo(&self) -> bool { self.identity_repo.is_some() }
    pub fn has_metadata_repo(&self) -> bool { self.metadata_repo.is_some() }
    pub fn has_settings_repo(&self) -> bool { self.settings_repo.is_some() }
    pub fn has_outbox_repo(&self) -> bool { self.outbox_repo.is_some() }

    pub fn with_account_id(mut self, id: AccountId) -> Self {
        self.account_id = Some(id);
        self
    }

    pub fn with_region(mut self, region: RegionCode) -> Self {
        self.region = Some(region);
        self
    }

    pub fn with_identity_repo(mut self, repo: Arc<dyn AccountIdentityRepository>) -> Self {
        self.identity_repo = Some(repo);
        self
    }

    pub fn with_outbox_repo(mut self, repo: Arc<dyn OutboxRepository>) -> Self {
        self.outbox_repo = Some(repo);
        self
    }

    pub fn with_metadata_repo(mut self, repo: Arc<dyn AccountMetadataRepository>) -> Self {
        self.metadata_repo = Some(repo);
        self
    }

    pub fn with_settings_repo(mut self, repo: Arc<dyn AccountSettingsRepository>) -> Self {
        self.settings_repo = Some(repo);
        self
    }

    pub fn with_pool(mut self, pool: sqlx::PgPool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn build(self) -> AccountContext {
        AccountContext::new(
            self.account_id.expect("account_id is required"),
            self.region.expect("region is required"),
            self.identity_repo.expect("identity_repo is required"),
            self.metadata_repo.expect("metadata_repo is required"),
            self.settings_repo.expect("settings_repo is required"),
            self.outbox_repo.expect("outbox_repo is required"),
            self.pool.expect("pool is required"),
        )
    }
}