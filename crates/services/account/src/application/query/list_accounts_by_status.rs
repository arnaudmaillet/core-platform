use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::AccountRepository;
use crate::application::query::get_account_by_id::AccountView;
use crate::domain::value_object::AccountStatus;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct AccountListView {
    pub accounts: Vec<AccountView>,
    pub total: i64,
}

#[derive(Debug, Clone)]
pub struct ListAccountsByStatusQuery {
    pub status: String,
    pub limit: i64,
    pub offset: i64,
}

impl Query for ListAccountsByStatusQuery {
    type Response = AccountListView;
}

pub struct ListAccountsByStatusHandler {
    repo: Arc<dyn AccountRepository>,
}

impl ListAccountsByStatusHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<ListAccountsByStatusQuery> for ListAccountsByStatusHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<ListAccountsByStatusQuery>,
    ) -> Result<AccountListView, Self::Error> {
        let cmd = &envelope.payload;

        let status = AccountStatus::try_from(cmd.status.as_str())
            .map_err(|_| AccountError::InvalidAccountStatus(cmd.status.clone()))?;

        let limit = cmd.limit.max(1).min(1000);
        let offset = cmd.offset.max(0);

        let (accounts, total) = tokio::try_join!(
            self.repo.list_by_status(&status, limit, offset),
            self.repo.count_by_status(&status),
        )?;

        Ok(AccountListView {
            accounts: accounts.iter().map(AccountView::from).collect(),
            total,
        })
    }
}
pub type ListAccountsByStatusResponse = AccountListView;
