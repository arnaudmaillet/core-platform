use std::sync::Arc;

use cqrs::{Envelope, Query, QueryHandler};

use crate::application::port::{ProfileRepository, ProfileSummary};
use crate::domain::value_object::AccountId;
use crate::error::ProfileError;

#[derive(Debug, Clone)]
pub struct ListProfilesByAccountQuery {
    pub account_id: String,
    /// Maximum number of profiles to return (capped at 100 internally).
    pub limit: u32,
    /// Opaque page cursor; `None` starts from the beginning.
    pub page_token: Option<String>,
}

impl Query for ListProfilesByAccountQuery {
    type Response = (Vec<ProfileSummary>, Option<String>);
}

pub struct ListProfilesByAccountHandler {
    repo: Arc<dyn ProfileRepository>,
}

impl ListProfilesByAccountHandler {
    pub fn new(repo: Arc<dyn ProfileRepository>) -> Self {
        Self { repo }
    }
}

impl QueryHandler<ListProfilesByAccountQuery> for ListProfilesByAccountHandler {
    type Error = ProfileError;

    async fn handle(&self, envelope: Envelope<ListProfilesByAccountQuery>) -> Result<(Vec<ProfileSummary>, Option<String>), Self::Error> {
        let account_id = AccountId::try_from(envelope.payload.account_id.as_str())?;
        let limit = envelope.payload.limit.clamp(1, 100) as i32;
        let page_token = envelope.payload.page_token.as_deref();

        self.repo.list_by_account(&account_id, limit, page_token).await
    }
}
