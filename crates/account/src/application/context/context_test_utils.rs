use std::sync::Arc;

use shared_kernel::domain::repositories::OutboxRepositoryStub;

use crate::{application::context::{AccountContext, AccountContextBuilder}, domain::repositories::{AccountIdentityRepositoryStub, AccountMetadataRepositoryStub, AccountSettingsRepositoryStub}};

// Dans un module ou fichier de test
pub trait AccountContextTestExt {
    fn build_test(self) -> AccountContext;
}

impl AccountContextTestExt for AccountContextBuilder {
    fn build_test(mut self) -> AccountContext {
        if !self.has_identity_repo() {
            self = self.with_identity_repo(Arc::new(AccountIdentityRepositoryStub::new()));
        }
        if !self.has_metadata_repo() {
            self = self.with_metadata_repo(Arc::new(AccountMetadataRepositoryStub::new()));
        }
        if !self.has_settings_repo() {
            self = self.with_settings_repo(Arc::new(AccountSettingsRepositoryStub::new()));
        }
        if !self.has_outbox_repo() {
            self = self.with_outbox_repo(Arc::new(OutboxRepositoryStub::new()));
        }
        
        self.build()
    }
}