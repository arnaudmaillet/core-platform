// crates/profile/src/infrastructure/postgres/rows/postgres_profile_row.rs

use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName, ProfileId, Handle, ProfileStats, SocialLinks};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::value_objects::{
    AccountId, Counter, LocationLabel, RegionCode, Url,
};
use shared_kernel::errors::{DomainError, Result};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow, Debug)]
pub struct PostgresProfileRow {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub region_code: String,
    pub display_name: String,
    pub handle: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub location_label: Option<String>,
    pub social_links: JsonValue,
    pub post_count: i64,
    pub is_private: bool,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}


impl From<&Profile> for PostgresProfileRow {
    fn from(p: &Profile) -> Self {
        Self {
            id: p.id().as_uuid(),
            owner_id: p.owner_id().as_uuid(),
            region_code: p.region_code().to_string(),
            display_name: p.display_name().to_string(),
            handle: p.handle().as_str().to_string(),
            bio: p.bio().as_ref().map(|b| b.to_string()),
            avatar_url: p.avatar_url().as_ref().map(|u| u.to_string()),
            banner_url: p.banner_url().as_ref().map(|u| u.to_string()),
            location_label: p.location_label().as_ref().map(|l| l.to_string()),
            social_links: serde_json::to_value(p.social_links()).unwrap_or(JsonValue::Null),
            post_count: p.post_count() as i64,
            is_private: p.is_private(),
            version: p.version() as i64,
            created_at: p.created_at(),
            updated_at: p.updated_at(),
        }
    }
}


impl TryFrom<PostgresProfileRow> for Profile {
    type Error = DomainError;

    fn try_from(row: PostgresProfileRow) -> Result<Self> {
        let social_links = serde_json::from_value::<Option<SocialLinks>>(row.social_links.clone())
            .map_err(|e| DomainError::Internal(format!("Failed to deserialize social_links: {}", e)))?;

        // --- Conversions sécurisées try_into ---
        // On convertit les i64 signés de la DB en u64 non-signés du domaine
        let post_count_u64: u64 = row.post_count.try_into()
            .map_err(|_| DomainError::Internal("Negative post_count in database".into()))?;

        let version_u64: u64 = row.version.try_into()
            .map_err(|_| DomainError::Internal("Negative version in database".into()))?;

        Ok(Profile::restore(
            ProfileId::from_uuid(row.id),
            AccountId::from_uuid(row.owner_id),
            RegionCode::from_raw(row.region_code),
            DisplayName::from_raw(row.display_name),
            Handle::from_raw(row.handle),
            row.bio.map(Bio::from_raw),
            row.avatar_url.map(Url::from_raw),
            row.banner_url.map(Url::from_raw),
            row.location_label.map(LocationLabel::from_raw),
            social_links,
            Counter::from_raw(post_count_u64),
            row.is_private,
            version_u64,
            row.created_at,
            row.updated_at,
        ))
    }
}