// crates/account/src/application/test_utils.rs (ou similaire)

use std::sync::Arc;

use shared_kernel::domain::repositories::OutboxRepositoryStub;
use shared_kernel::domain::value_objects::AccountId;
use crate::application::context::{AccountContext, AccountContextTestExt};
use crate::domain::repositories::{AccountIdentityRepositoryStub, AccountMetadataRepositoryStub, AccountSettingsRepositoryStub};


pub struct TestFixture<Usecase> {
    use_case: Usecase,
    ctx: AccountContext,
    identity_repo: Arc<AccountIdentityRepositoryStub>,
    metadata_repo: Arc<AccountMetadataRepositoryStub>,
    settings_repo: Arc<AccountSettingsRepositoryStub>,
    outbox_repo: Arc<OutboxRepositoryStub>,
}

impl<Usecase> TestFixture<Usecase> {
    pub fn new<F>(use_case_factory: F) -> Self 
    where F: FnOnce() -> Usecase 
    {
        let identity_repo = Arc::new(AccountIdentityRepositoryStub::new());
        let metadata_repo = Arc::new(AccountMetadataRepositoryStub::new());
        let settings_repo = Arc::new(AccountSettingsRepositoryStub::new());
        let outbox_repo = Arc::new(OutboxRepositoryStub::new());
        
        let account_id = AccountId::new();
        let ctx = AccountContext::builder()
            .with_account_id(account_id)
            .with_identity_repo(identity_repo.clone())
            .with_metadata_repo(metadata_repo.clone())
            .with_settings_repo(settings_repo.clone())
            .with_outbox_repo(outbox_repo.clone())
            .build_test();

        Self {
            use_case: use_case_factory(),
            ctx,
            identity_repo,
            metadata_repo,
            settings_repo,
            outbox_repo,
        }
    }

    // Helpers communs à TOUS les tests
    pub fn account_id(&self) -> AccountId {
        self.ctx.account_id().clone()
    }

    pub fn region(&self) -> RegionCode {
        self.ctx.region().clone()
    }

    pub fn ctx(&self) -> &AccountContext {
        &self.ctx
    }

    pub fn use_case(&self) -> &Usecase {
        &self.use_case
    }

    pub fn outbox_count(&self) -> usize { 
        self.outbox_repo.saved_events.lock().unwrap().len() 
    }

    pub fn identity_repo(&self) -> &AccountIdentityRepositoryStub {
        &self.identity_repo
    }

    pub fn metadata_repo(&self) -> &AccountMetadataRepositoryStub {
        &self.metadata_repo
    }

    pub fn settings_repo(&self) -> &AccountSettingsRepositoryStub {
        &self.settings_repo
    }

    pub fn outbox_events(&self) -> Vec<String> {
        self.outbox_repo.saved_events.lock().unwrap().clone()
    }
}