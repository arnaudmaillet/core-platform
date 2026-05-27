// crates/profile/src/infrastructure/postgres/rows/postgres_profile_row.rs

use crate::domain::entities::Profile;
use crate::domain::types::{Bio, DisplayName, Handle, Location, Socials};
use chrono::{DateTime, Utc};
use infra_sqlx::sqlx::FromRow;
use serde_json::Value as JsonValue;
use shared_kernel::{
    core::{Error, Identifier, Result, Versioned},
    types::{AccountId, ProfileId, Url},
};
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct PostgresProfileRow {
    pub profile_id: Uuid,
    pub account_id: Uuid,
    pub region: String,
    pub display_name: String,
    pub handle: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub location_label: Option<String>,
    pub social_links: Option<JsonValue>,
    pub is_private: bool,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl PostgresProfileRow {
    pub fn from_domain(p: &Profile) -> Self {
        Self {
            profile_id: p.profile_id().as_uuid(),
            account_id: p.account_id().uuid().clone(),
            region: p.account_id().region().as_str().to_string(),
            display_name: p.display_name().to_string(),
            handle: p.handle().as_str().to_string(),
            bio: p.bio().as_ref().map(|b| b.to_string()),
            avatar_url: p.avatar().as_ref().map(|u| u.to_string()),
            banner_url: p.banner().as_ref().map(|u| u.to_string()),
            location_label: p.location().as_ref().map(|l| l.to_string()),
            social_links: Some(serde_json::to_value(p.socials()).unwrap_or(JsonValue::Null)),
            is_private: p.is_private(),
            version: p.version() as i64,
            created_at: p.created_at(),
            updated_at: p.updated_at(),
        }
    }

    pub fn to_domain(self) -> Result<Profile> {
        let raw_json = self.social_links.unwrap_or(JsonValue::Null);

        let social_links = serde_json::from_value::<Option<Socials>>(raw_json)
            .map_err(|e| Error::internal(format!("Failed to deserialize social_links: {}", e)))?;

        let version_u64: u64 = self
            .version
            .try_into()
            .map_err(|_| Error::internal("Negative version in database"))?;

        let account_id = AccountId::new(self.account_id);
        let profile_id = ProfileId::from_uuid(self.profile_id);

        Ok(Profile::restore(
            profile_id,
            account_id,
            DisplayName::from_raw(self.display_name),
            Handle::from_raw(self.handle),
            self.bio.map(Bio::from_raw),
            self.avatar_url.map(Url::from_raw),
            self.banner_url.map(Url::from_raw),
            self.location_label.map(Location::from_raw),
            social_links,
            self.is_private,
            version_u64,
            self.created_at,
            self.updated_at,
        ))
    }
}
