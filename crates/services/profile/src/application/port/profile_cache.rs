use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::aggregate::Profile;
use crate::domain::entity::ProfileLink;
use crate::domain::value_object::{AccountId, ProfileId};
use crate::error::ProfileError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileLinkView {
    pub label: String,
    pub url: String,
}

impl From<&ProfileLink> for ProfileLinkView {
    fn from(l: &ProfileLink) -> Self {
        Self {
            label: l.label.clone(),
            url: l.url.as_str().to_owned(),
        }
    }
}

/// Full serialized profile view cached in Redis at key `profile:v1:{id}`.
///
/// All value objects are flattened to primitives so the cache layer has zero
/// dependency on the domain module — any service can deserialize this view
/// from Redis without importing the profile crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileView {
    pub id: String,
    pub account_id: String,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub website_url: Option<String>,
    pub custom_links: Vec<ProfileLinkView>,
    pub profile_kind: String,
    pub visibility: String,
    pub verified: bool,
    pub verification_kind: Option<String>,
    pub locale: String,
    pub timezone: Option<String>,
    pub status: String,
    pub masked_at: Option<DateTime<Utc>>,
    pub masking_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: i64,
}

impl From<&Profile> for ProfileView {
    fn from(p: &Profile) -> Self {
        Self {
            id: p.id().as_str(),
            account_id: p.account_id().as_str(),
            handle: p.handle().as_str().to_owned(),
            display_name: p.display_name().as_str().to_owned(),
            bio: p.bio().map(|b| b.as_str().to_owned()),
            avatar_url: p.avatar_url().map(|u| u.as_str().to_owned()),
            banner_url: p.banner_url().map(|u| u.as_str().to_owned()),
            website_url: p.website_url().map(|u| u.as_str().to_owned()),
            custom_links: p.custom_links().iter().map(ProfileLinkView::from).collect(),
            profile_kind: p.profile_kind().as_str().to_owned(),
            visibility: p.visibility().as_str().to_owned(),
            verified: p.verified(),
            verification_kind: p.verification_kind().map(|v| v.as_str().to_owned()),
            locale: p.locale().as_str().to_owned(),
            timezone: p.timezone().map(str::to_owned),
            status: p.status().as_str().to_owned(),
            masked_at: p.masked_at(),
            masking_reason: p.masking_reason().map(|r| r.as_str().to_owned()),
            created_at: p.created_at(),
            updated_at: p.updated_at(),
            version: p.version(),
        }
    }
}

/// Cache port for the profile read path.
///
/// Three independent Redis key namespaces with separate TTLs:
/// - `profile:v1:{id}` — full ProfileView, TTL 300 s.
/// - `handle:v1:{handle}` — profile_id string, TTL 600 s.
/// - `account:profiles:v1:{account_id}` — evicted on writes; no SET, only DEL.
#[async_trait]
pub trait ProfileCache: Send + Sync + 'static {
    async fn get_by_id(&self, id: &ProfileId) -> Result<Option<ProfileView>, ProfileError>;
    async fn set_by_id(&self, view: &ProfileView) -> Result<(), ProfileError>;
    async fn invalidate_by_id(&self, id: &ProfileId) -> Result<(), ProfileError>;

    async fn get_profile_id_by_handle(
        &self,
        handle: &str,
    ) -> Result<Option<ProfileId>, ProfileError>;
    async fn set_handle_mapping(
        &self,
        handle: &str,
        id: ProfileId,
    ) -> Result<(), ProfileError>;
    async fn invalidate_handle(&self, handle: &str) -> Result<(), ProfileError>;

    async fn invalidate_account_profiles(
        &self,
        account_id: &AccountId,
    ) -> Result<(), ProfileError>;
}
