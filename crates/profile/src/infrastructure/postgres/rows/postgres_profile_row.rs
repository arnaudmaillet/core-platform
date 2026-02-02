// crates/profile/src/infrastructure/postgres/rows/postgres_profile_row.rs

use chrono::{DateTime, Utc};
use sqlx::FromRow;
use serde_json::Value as JsonValue;
use uuid::Uuid;
use shared_kernel::domain::events::AggregateRoot;
use shared_kernel::domain::Identifier;
use shared_kernel::domain::value_objects::{Counter, LocationLabel, RegionCode, Url, AccountId, Username};
use shared_kernel::errors::{Result, DomainError};
use crate::domain::builders::ProfileBuilder;
use crate::domain::entities::Profile;
use crate::domain::value_objects::{Bio, DisplayName};

#[derive(FromRow)]
pub struct PostgresProfileRow {
    pub account_id: Uuid,
    pub region_code: String,
    pub display_name: String,
    pub username: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub banner_url: Option<String>,
    pub location_label: Option<String>,
    pub social_links: JsonValue,
    pub post_count: i64,
    pub is_private: bool,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TryFrom<PostgresProfileRow> for Profile {
    type Error = DomainError;

    fn try_from(row: PostgresProfileRow) -> Result<Self> {
        // Transformation des types techniques en Value Objects (Chemin rapide)
        let social_links = serde_json::from_value(row.social_links)
            .map_err(|e| DomainError::Validation {
                field: "social_links",
                reason: e.to_string()
            })?;

        // Reconstruction de l'agrégat via la méthode de restauration
        let profile = ProfileBuilder::restore(
            AccountId::from_uuid(row.account_id),
            RegionCode::from_raw(row.region_code),
            DisplayName::from_raw(row.display_name),
            Username::from_raw(row.username),
            row.bio.map(Bio::from_raw),
            row.avatar_url.map(Url::from_raw),
            row.banner_url.map(Url::from_raw),
            row.location_label.map(LocationLabel::from_raw),
            social_links,
            Counter::try_from(row.post_count)?,
            row.is_private,
            row.version,
            row.created_at,
            row.updated_at,
        );

        Ok(profile)
    }
}

impl From<&Profile> for PostgresProfileRow {
    fn from(p: &Profile) -> Self {
        Self {
            account_id: p.account_id().as_uuid(),
            region_code: p.region_code().to_string(),
            display_name: p.display_name().to_string(),
            username: p.username().to_string(),
            bio: p.bio().as_ref().map(|b| b.to_string()),
            avatar_url: p.avatar_url().as_ref().map(|u| u.to_string()),
            banner_url: p.banner_url().as_ref().map(|u| u.to_string()),
            location_label: p.location_label().as_ref().map(|l| l.to_string()),
            social_links: serde_json::to_value(&p.social_links()).unwrap_or(JsonValue::Null),
            post_count: p.post_count() as i64,
            is_private: p.is_private(),
            version: p.version(),
            created_at: p.created_at(),
            updated_at: p.updated_at(),
        }
    }
}