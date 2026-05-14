// crates/account/src/application/context_builder.rs

use std::sync::Arc;

use crate::{
    application::context::{AccountAppContext, AccountContext},
    domain::repositories::AccountRepository,
};
use shared_kernel::{
    messaging::OutboxRepository,
    types::{AccountId, RegionCode},
};

pub struct AccountContextBuilder {
    app: Option<AccountAppContext>,
    account_id: Option<AccountId>,
    region: Option<RegionCode>,
}

impl AccountContextBuilder {
    pub fn new() -> Self {
        Self {
            app: None,
            account_id: None,
            region: None,
        }
    }

    pub fn account_id(&self) -> Option<&AccountId> {
        self.account_id.as_ref()
    }

    pub fn region(&self) -> Option<&RegionCode> {
        self.region.as_ref()
    }

    pub fn app(&self) -> Option<&AccountAppContext> {
        self.app.as_ref()
    }

    pub fn account_repo(&self) -> Option<Arc<dyn AccountRepository>> {
        self.app.as_ref().map(|a| a.account_repo())
    }

    pub fn outbox_repo(&self) -> Option<Arc<dyn OutboxRepository>> {
        self.app.as_ref().map(|a| a.outbox_repo())
    }

    pub fn with_app(mut self, app: AccountAppContext) -> Self {
        self.app = Some(app);
        self
    }

    pub fn with_account_id(mut self, id: AccountId) -> Self {
        self.account_id = Some(id);
        self
    }

    pub fn with_region(mut self, region: RegionCode) -> Self {
        self.region = Some(region);
        self
    }

    pub fn build(self) -> AccountContext {
        AccountContext::new(
            self.app
                .expect("AccountAppContext is required. Use .with_app()"),
            self.account_id
                .expect("account_id is required for AccountContext"),
        )
    }
}

impl Default for AccountContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}
