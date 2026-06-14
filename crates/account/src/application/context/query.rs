// crates/account/src/application/context/query_context.rs

use crate::application::context::AccountKernelCtx;
use crate::domain::entities::Account;
use shared_kernel::{
    core::Result,
    types::{AccountId, Region},
};

#[derive(Clone)]
pub struct AccountQueryCtx {
    kernel: AccountKernelCtx,
    region: Region,
}

impl AccountQueryCtx {
    pub(crate) fn new(kernel: AccountKernelCtx, region: Region) -> Self {
        Self { kernel, region }
    }

    pub fn region(&self) -> Region {
        self.region
    }

    pub async fn find_by_id(&self, account_id: AccountId) -> Result<Option<Account>> {
        self.kernel
            .account_repo()
            .find_by_id(self.region, account_id, None)
            .await
    }
}
