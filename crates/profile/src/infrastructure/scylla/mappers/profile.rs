// crates/profile/src/infrastructure/scylla/rows/scylla_profile_row.rs

use crate::domain::entities::Profile;
use crate::domain::types::{Bio, DisplayName, Handle, Location, Socials};
use chrono::{DateTime, Utc};
use infra_scylla::scylla;
use infra_scylla::scylla::value::CqlTimestamp;
use shared_kernel::{
    core::{Error, Identifier, Result, Versioned},
    types::{AccountId, ProfileId, Url},
};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(scylla::DeserializeRow, Debug, Clone)]
pub struct CqlProfileByAccountRow {
    pub account_id: Uuid,
    pub profile_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_private: bool,
}

#[derive(scylla::DeserializeRow, Debug, Clone)]
pub struct CqlProfileRow {
    pub id: Uuid,
    pub account_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub location_label: Option<String>,
    pub social_links: HashMap<String, String>,
    pub is_private: bool,
    pub version: i64,
    pub created_at: CqlTimestamp,
    pub updated_at: CqlTimestamp,
}

impl CqlProfileRow {
    pub fn from_domain(p: &Profile) -> Self {
        let social_links_map = p
            .socials()
            .cloned()
            .map(|s| s.into_map())
            .unwrap_or_default();

        Self {
            id: p.profile_id().as_uuid(),
            account_id: p.account_id().uuid().clone(),
            handle: p.handle().as_str().to_string(),
            display_name: p.display_name().to_string(),
            bio: p.bio().as_ref().map(|b| b.to_string()),
            avatar_url: p.avatar().as_ref().map(|u| u.to_string()),
            banner_url: p.banner().as_ref().map(|u| u.to_string()),
            location_label: p.location().as_ref().map(|l| l.to_string()),
            social_links: social_links_map,
            is_private: p.is_private(),
            version: p.version() as i64,
            created_at: CqlTimestamp(p.created_at().timestamp_millis()),
            updated_at: CqlTimestamp(p.updated_at().timestamp_millis()),
        }
    }

    pub fn to_domain(self) -> Result<Profile> {
        let version_u64: u64 = self
            .version
            .try_into()
            .map_err(|_| Error::internal("Negative version in database"))?;

        let created_dt = DateTime::<Utc>::from_timestamp_millis(self.created_at.0)
            .ok_or_else(|| Error::internal("Invalid created_at timestamp"))?;
        let updated_dt = DateTime::<Utc>::from_timestamp_millis(self.updated_at.0)
            .ok_or_else(|| Error::internal("Invalid updated_at timestamp"))?;

        Ok(Profile::restore(
            ProfileId::from_uuid(self.id),
            AccountId::new(self.account_id),
            DisplayName::from_raw(self.display_name),
            Handle::from_raw(self.handle),
            self.bio.map(Bio::from_raw),
            self.avatar_url.map(Url::from_raw),
            self.banner_url.map(Url::from_raw),
            self.location_label.map(Location::from_raw),
            Some(Socials::from_map(self.social_links)),
            self.is_private,
            version_u64,
            created_dt,
            updated_dt,
        ))
    }
}
