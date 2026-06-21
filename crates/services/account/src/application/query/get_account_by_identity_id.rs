use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::AccountRepository;
use crate::application::query::get_account_by_id::AccountView;
use crate::domain::value_object::IdentityId;
use crate::error::AccountError;

#[derive(Debug, Clone)]
pub struct GetAccountByIdentityIdQuery {
    pub identity_id: String,
}

impl Query for GetAccountByIdentityIdQuery {
    type Response = AccountView;
}

pub struct GetAccountByIdentityIdHandler {
    repo: Arc<dyn AccountRepository>,
}

impl GetAccountByIdentityIdHandler {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<GetAccountByIdentityIdQuery> for GetAccountByIdentityIdHandler {
    type Error = AccountError;

    async fn handle(
        &self,
        envelope: Envelope<GetAccountByIdentityIdQuery>,
    ) -> Result<AccountView, Self::Error> {
        let identity_id = IdentityId::new(envelope.payload.identity_id.clone())?;
        let account = self
            .repo
            .find_by_identity_id(&identity_id)
            .await?
            .ok_or_else(|| AccountError::AccountNotFound {
                id: envelope.payload.identity_id.clone(),
            })?;
        Ok(AccountView::from(&account))
    }
}
pub type GetAccountByIdentityIdResponse = AccountView;
