use crate::application::context::AccountAppContext;
use crate::domain::entities::Account;
use shared_kernel::{
    core::Result,
    types::{AccountId, Region},
};

#[derive(Clone)]
pub struct AccountQueryContext {
    app: AccountAppContext,
    region: Region,
}

impl AccountQueryContext {
    pub(crate) fn new(app: AccountAppContext, region: Region) -> Self {
        Self { app, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>> {
        self.app
            .account_repo()
            .find_by_id(self.region, account_id, None)
            .await
    }
}
