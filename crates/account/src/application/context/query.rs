use crate::application::context::AccountAppContext;
use crate::domain::entities::Account;
use shared_kernel::{
    core::Result,
    types::{AccountId, Region},
};

pub struct AccountQueryContext<TM> {
    app: AccountAppContext<TM>,
    region: Region,
}

impl<TM> Clone for AccountQueryContext<TM> {
    fn clone(&self) -> Self {
        Self {
            app: self.app.clone(),
            region: self.region,
        }
    }
}

impl<TM> AccountQueryContext<TM> {
    pub(crate) fn new(app: AccountAppContext<TM>, region: Region) -> Self {
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
