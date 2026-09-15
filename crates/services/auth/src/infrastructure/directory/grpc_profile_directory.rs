use async_trait::async_trait;
use profile_api::profile_service_client::ProfileServiceClient;
use profile_api::ListProfilesByAccountRequest;
use tonic::transport::Channel;
use tonic::Code;
use tracing::instrument;

use crate::application::port::ProfileDirectory;
use crate::domain::value_object::{AccountId, ProfileId};
use crate::error::AuthError;

/// Page size per `ListProfilesByAccount` call.
const PAGE_SIZE: i32 = 64;
/// Hard cap on the profiles minted into one token (bounds the JWT size; an account
/// with more profiles than this is a product anomaly, logged at warn).
const MAX_PROFILES: usize = 256;

/// gRPC implementation of [`ProfileDirectory`], backed by the `profile` service.
#[derive(Clone)]
pub struct GrpcProfileDirectory {
    client: ProfileServiceClient<Channel>,
}

impl GrpcProfileDirectory {
    pub fn new(channel: Channel) -> Self {
        Self { client: ProfileServiceClient::new(channel) }
    }
}

#[async_trait]
impl ProfileDirectory for GrpcProfileDirectory {
    #[instrument(name = "auth.directory.profiles", skip(self), fields(account.id = %account_id))]
    async fn list_profile_ids(&self, account_id: &AccountId) -> Result<Vec<ProfileId>, AuthError> {
        let mut client = self.client.clone();
        let mut ids = Vec::new();
        let mut page_token = String::new();

        loop {
            let page = client
                .list_profiles_by_account(ListProfilesByAccountRequest {
                    account_id: account_id.as_str(),
                    limit: PAGE_SIZE,
                    page_token: page_token.clone(),
                })
                .await
                .map_err(|status| match status.code() {
                    // No profiles yet is not an outage: the account may act as none.
                    Code::NotFound => AuthError::DomainViolation {
                        field: "account_id".into(),
                        message: "no profiles".into(),
                    },
                    _ => AuthError::ProfileDirectoryUnavailable,
                });
            let page = match page {
                Ok(page) => page.into_inner(),
                Err(AuthError::DomainViolation { .. }) => break,
                Err(other) => return Err(other),
            };

            for view in page.profiles {
                match ProfileId::try_from(view.profile_id.as_str()) {
                    Ok(id) => ids.push(id),
                    // A malformed id from the SoR is skipped, not fatal: it would
                    // otherwise strip every grant for one bad row.
                    Err(_) => tracing::warn!(profile.id = %view.profile_id, "skipping non-UUID profile id"),
                }
                if ids.len() >= MAX_PROFILES {
                    tracing::warn!(account.id = %account_id, cap = MAX_PROFILES, "profile grants capped");
                    return Ok(ids);
                }
            }

            if page.next_page_token.is_empty() {
                break;
            }
            page_token = page.next_page_token;
        }

        Ok(ids)
    }
}
